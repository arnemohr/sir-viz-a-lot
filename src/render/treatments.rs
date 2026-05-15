//! P1.2.2 — `TreatmentPipeline` render integration.
//!
//! A *treatment* runs **before** the existing effect chain: per-frame layer
//! order is `raster source → treatment → effects → warp → compositor`. The
//! treatment stage writes into the effect chain's first ping-pong texture,
//! replacing the [`SvgLayerPipeline`](crate::svg_layer::render::SvgLayerPipeline)
//! blit on layers that carry a [`Treatment`](crate::project::schema::Treatment).
//!
//! This module scaffolds the dispatch + registry. v0.4 / Phase 1 W2 ships
//! exactly one preset — `"identity"` — which proves the bind-group contract
//! by performing a no-op blit through a separate pipeline (not the
//! `svg_pipeline` shared with the default path). W3 grows the registry into
//! the real preset library (`tone_map`, `blur_mask`, `luminance_reveal`,
//! `texture_overlay`, `palette_extract`, `collage`).
//!
//! # Default-path invariant
//!
//! When `layer.treatment.is_none()` *or* the treatment's `preset_id` is not
//! registered, this module is never reached — the caller falls back to the
//! pre-P1.2.2 `svg_pipeline.render` blit. The default path therefore stays
//! bit-exact identical, which is the core P1.2.2 acceptance criterion.
//!
//! # Unknown-preset behaviour
//!
//! `dispatch` returns `false` for an unregistered `preset_id`. The caller in
//! `app.rs` interprets that as "skip the treatment but still render the
//! source image" — i.e. an Image / Video layer with a bad preset_id renders
//! as if `treatment: None`, which is the user-forgiving choice. The audit
//! (W1) already emits a `Warn` for the empty / mistyped case so the
//! operator sees a UI hint; the renderer chooses not to make the layer
//! invisible. This deliberately diverges from the `FxLayer` unknown-preset
//! path (which hides the layer entirely) because an Image layer's *content*
//! is meaningful on its own.

use std::collections::HashMap;

/// Identity preset: a no-op blit that proves the dispatch contract.
/// Renders the source texture into the destination using the same
/// `textured_quad.wgsl` shader the default `svg_pipeline` path uses, but
/// through a separate pipeline owned by this module.
pub const IDENTITY_PRESET_ID: &str = "identity";

/// `tone_map` preset id (P1.3.1).
pub const TONE_MAP_PRESET_ID: &str = "tone_map";

/// `luminance_reveal` preset id (P1.3.3).
pub const LUMINANCE_REVEAL_PRESET_ID: &str = "luminance_reveal";

/// `blur_mask` preset id (P1.3.2).
pub const BLUR_MASK_PRESET_ID: &str = "blur_mask";

/// `texture_overlay` preset id (P1.3.4).
pub const TEXTURE_OVERLAY_PRESET_ID: &str = "texture_overlay";

/// `palette_extract` preset id (P1.3.5).
pub const PALETTE_EXTRACT_PRESET_ID: &str = "palette_extract";

/// `collage` preset id (P1.3.6).
pub const COLLAGE_PRESET_ID: &str = "collage";

/// `displacement_ripple` preset id (P2.4.1).
pub const DISPLACEMENT_RIPPLE_PRESET_ID: &str = "displacement_ripple";

/// `refraction` preset id (P2.4.2).
pub const REFRACTION_PRESET_ID: &str = "refraction";

/// PCleanup.2.1 — `ripple_lens` preset id (first W2 sibling treatment
/// per the source-modifier-placement decision doc). Concentric-ring
/// UV displacement keyed to SDF distance — the SourceModifier sibling
/// of the generative `mask_edge_ripple_wash` FX preset.
pub const RIPPLE_LENS_PRESET_ID: &str = "ripple_lens";

/// PCleanup.2.2 — `edge_lens` preset id. N traveling refraction bumps
/// orbiting the mask boundary — the SourceModifier sibling of the
/// generative `mask_edge_wave_wash` FX preset.
pub const EDGE_LENS_PRESET_ID: &str = "edge_lens";

/// PCleanup.2.7 — `field_advect_source` preset id. Advects the source
/// image along the SDF gradient field — the SourceModifier sibling of
/// the generative `mask_field_flow` FX preset.
pub const FIELD_ADVECT_PRESET_ID: &str = "field_advect_source";

/// PCleanup.1.2 — `fluid_warp` preset id. Warps the source image using a
/// bounded-fluid velocity field — the SourceModifier re-path of the
/// originally-deferred FX preset (commit 2a30578, decision 920c8c2).
pub const FLUID_WARP_PRESET_ID: &str = "fluid_warp";

/// PCleanup.2.3 — `fluid_warp_full` preset id. Unbounded sibling of
/// `fluid_warp`: uses the `fluid_identity` compute pass (no SDF boundary),
/// so the warp covers the whole layer rect regardless of mask shape.
/// Works on any layer source (Image / Video / SVG / FxLayer).
pub const FLUID_WARP_FULL_PRESET_ID: &str = "fluid_warp_full";

/// PCleanup.2.9 — `zone_brighten` preset id. Fifth W2 sibling treatment.
/// Multiplicatively boosts the luminance of the source image inside the
/// layer's ZONE_WINDOW-tagged area, with the same exponential edge falloff
/// as `fx_zone_light_spill`. Outside ZONE_WINDOW, source passes through
/// unchanged. `intensity = 0.0` is a bit-exact passthrough.
pub const ZONE_BRIGHTEN_PRESET_ID: &str = "zone_brighten";

/// PCleanup.2.10 — `zone_lens` preset id. Sixth W2 sibling treatment.
/// Displaces the source image's UV coordinates in a thin band around the
/// ZONE_WINDOW mask edge, creating a lens / refraction effect at the zone
/// perimeter. Mirrors `fx_zone_edge_ripple`'s spatial band shape but reads
/// `t_source` instead of generating new pixels. `amplitude = 0.0` is a
/// bit-exact passthrough.
pub const ZONE_LENS_PRESET_ID: &str = "zone_lens";

/// PCleanup.2.4 — `spotlights` preset id. Seventh W2 sibling treatment.
/// Particles drift slowly inside the layer's mask (or over the full layer
/// rect when no mask is present) and boost source luminance with a Gaussian
/// falloff around each particle position.  The SourceModifier sibling of
/// the generative `particles_identity` FX preset.
/// `brightness_gain = 0.0` is a bit-exact passthrough.
pub const SPOTLIGHTS_PRESET_ID: &str = "spotlights";

/// PCleanup.2.5a — `drift_pinholes` preset id. Eighth W2 sibling treatment.
/// Same particle compute pass as `spotlights`; the fragment pass inverts
/// the visibility — source pixels under particles stay visible, everywhere
/// else fades to black.  The effect resembles peepholes drifting over the
/// photo.  `opacity = 0.0` is a bit-exact passthrough; `opacity = 1.0` is
/// fully masked.
pub const DRIFT_PINHOLES_PRESET_ID: &str = "drift_pinholes";

/// PCleanup.2.5b — `drift_brushstrokes` preset id. Ninth W2 sibling treatment.
/// Companion of `drift_pinholes` — same particle compute pass (which now
/// writes per-particle velocity in UV/s), different fragment math.  Each
/// particle leaves a motion-blurred brushstroke trailing along its velocity
/// vector; source is visible inside the brushstroke and fades to black
/// elsewhere.  `opacity = 0.0` is bit-exact passthrough.  `smear_duration`
/// (seconds) controls how long the trail extends behind each particle.
pub const DRIFT_BRUSHSTROKES_PRESET_ID: &str = "drift_brushstrokes";

/// PCleanup.2.6 — `edge_sparks` preset id. Tenth W2 sibling treatment.
/// Sibling of `mask_edge_emission` — particles spawn at the mask edge,
/// drift outward along the SDF gradient, and additively brighten the
/// source pixels they pass over (no opaque dots; underlying detail still
/// visible).  Each spark has a finite lifetime (`lifetime_s`) and respawns
/// at a new edge point after expiring.  `brightness_gain = 0.0` is a
/// bit-exact passthrough.
pub const EDGE_SPARKS_PRESET_ID: &str = "edge_sparks";

/// PCleanup.2.8 — `collision_ripples` preset id. Eleventh W2 sibling.
/// Particles drift in the mask; when one would cross the boundary it
/// freezes at the collision point and starts a circular ripple that
/// radially displaces source UVs.  After a configurable lifetime the
/// ripple expires and the particle respawns.  Implementation is fully
/// GPU-resident: the existing per-particle SSBO encodes drift-vs-rippling
/// state in `_pad` (≥ 0.5 = active ripple, < 0.5 = drifting), so no CPU
/// readback or second buffer is needed.  `amplitude = 0.0` is bit-exact
/// passthrough — displacement collapses to zero and the fragment samples
/// the source at the original UV.
pub const COLLISION_RIPPLES_PRESET_ID: &str = "collision_ripples";

/// PCleanup.2.11 — `portal_warp` preset id. Twelfth (and final) W2 sibling.
/// Particles drift through the mask (shared spotlights compute) and the
/// fragment pass smears source UVs toward (or away from) each nearby
/// particle by a Gaussian magnitude, producing a "ghost through the room"
/// warp that travels with the particles.  `amplitude = 0.0` is bit-exact
/// passthrough.
pub const PORTAL_WARP_PRESET_ID: &str = "portal_warp";

/// Fixed at 4 (a 2×2 grid) — true variable-N collage requires either
/// dynamically-built bind groups or a texture array binding, deferred
/// to Phase 7.
pub const COLLAGE_SLOTS: usize = 4;

/// Inputs threaded into every preset's `render` call. The struct grows over
/// time (W3 adds `overlay` and `collage`); existing presets ignore fields
/// they don't read.
pub struct TreatmentInputs<'a> {
    /// Source texture view (post-raster, pre-effect). For Image layers this
    /// is the uploaded RGBA texture; for Video, the per-frame upload from
    /// the AVFoundation worker; for SVG, the resvg pixmap upload.
    pub source: &'a wgpu::TextureView,

    /// Per-layer fit-mode uniform (16 bytes: `[fit_mode, aspect_layer,
    /// focal_x, focal_y]`). Caller has already written the current frame's
    /// values via `queue.write_buffer` before calling `dispatch`.
    pub fit_uniform: &'a wgpu::Buffer,

    /// Free-form per-preset params from `Treatment.params`. Each preset
    /// documents which keys it reads via [`param_descriptors`]; missing
    /// keys fall back to the descriptor's documented default.
    #[allow(dead_code)] // identity ignores params; W3 presets will read this
    pub params: &'a HashMap<String, f32>,

    /// Frame-time scalar (`Clock::elapsed().as_secs_f32()`). Only consumed
    /// by time-varying presets; identity ignores it.
    #[allow(dead_code)] // W3 presets will consume this
    pub clock_secs: f32,

    /// Optional secondary texture for overlay-style presets
    /// (`texture_overlay`). `None` for presets that don't take an overlay.
    /// W3 wires this against `Treatment.overlay_path`.
    #[allow(dead_code)] // W3 will populate this
    pub overlay: Option<&'a wgpu::TextureView>,

    /// Slot textures for collage-style presets. Empty for presets that
    /// don't take a collage. W3 wires this against `Treatment.collage_paths`.
    #[allow(dead_code)] // W3 will populate this
    pub collage: &'a [&'a wgpu::TextureView],

    /// Layer's per-frame SDF (R32Float). Populated by the caller after
    /// `sync_mesh_and_mask`. `blur_mask` consumes this to gate the
    /// gaussian radius by distance-to-edge; other presets ignore it.
    pub sdf: Option<&'a wgpu::TextureView>,

    /// Layer's scratch texture (same format as the effect chain's
    /// ping-pong). `blur_mask` uses it for the horizontal-pass output
    /// before the vertical pass writes back to `dst`. Other multi-pass
    /// treatments may consume it the same way; single-pass presets
    /// leave it `None`.
    pub intermediate: Option<&'a wgpu::TextureView>,

    /// Layer's zone role (from `cfg.warp.zone_role`). Used by zone-aware
    /// treatments (`zone_brighten`, `zone_lens`). Other treatments ignore
    /// this field. `None` maps to `ZONE_NONE` (u32 0) in the shader
    /// uniform, which triggers the passthrough branch.
    pub zone_role: Option<crate::project::schema::ZoneRole>,

    /// PCleanup.2.4 — RNG seed for particle-based Treatments (spotlights
    /// and future W2.5/W2.6 siblings). Callers pass the layer's stable
    /// identifier (e.g. `LayerState::layer_id.0`) so particles seed
    /// deterministically per-layer. Non-particle treatments ignore this.
    ///
    /// Packing convention: `(seed as u32 & 0x7f_ffff) as f32` (lower 23
    /// bits → f32 mantissa). Mirrors `FxComputePipeline` convention.
    ///
    /// Bind-group slot 7 is reserved for the particle SSBO consumed by
    /// compute Treatments; slots 0–6 are taken by the existing layout
    /// (source, sampler, fit, params, sdf, sdf_sampler, zone). Future
    /// compute Treatments must use slot 7 to coexist with the others.
    #[allow(dead_code)] // consumed by spotlights; non-particle presets ignore
    pub seed: u64,

    /// PCleanup.2.4 — seconds since the project clock at which this
    /// layer was added. Used with `clock_secs` to compute per-layer
    /// local time for particle animation. For Image/Video layers this
    /// defaults to `0.0` (particles animate from project start).
    /// Non-particle treatments ignore this.
    #[allow(dead_code)] // consumed by spotlights; non-particle presets ignore
    pub t_layer_added_secs: f32,
}

/// Static descriptor for a tunable preset parameter. The Selected-layer UI
/// (P1.2.3) uses this metadata to render per-key sliders with the right
/// label + range + default.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // fields read by P1.2.3 UI scaffold
pub struct ParamDescriptor {
    /// HashMap key under `Treatment.params`.
    pub key: &'static str,
    /// Human-readable label shown next to the slider.
    pub label: &'static str,
    /// Slider min (inclusive).
    pub min: f32,
    /// Slider max (inclusive).
    pub max: f32,
    /// Default value when the key is missing from `Treatment.params`.
    pub default: f32,
}

// 004-T1.10 follow-up — the v2 Treatment-section picker's binary grouping
// (source-modifying vs generative/utility) is superseded by the Look chain's
// six-group `IntentGroup` taxonomy in `src/effects/mod.rs`. The non-test
// callers died with T1.30; the enum + fn stay only because the per-preset
// tests below still classify against them. `#[allow(dead_code)]` until
// those tests migrate to assert `IntentGroup` instead.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreatmentGroup {
    SourceModifier,
    GenerativeOrUtility,
}

#[allow(dead_code)]
pub fn treatment_group(preset_id: &str) -> TreatmentGroup {
    match preset_id {
        FLUID_WARP_PRESET_ID
        | FLUID_WARP_FULL_PRESET_ID
        | RIPPLE_LENS_PRESET_ID
        | EDGE_LENS_PRESET_ID
        | FIELD_ADVECT_PRESET_ID
        | ZONE_BRIGHTEN_PRESET_ID
        | ZONE_LENS_PRESET_ID
        | SPOTLIGHTS_PRESET_ID
        | DRIFT_PINHOLES_PRESET_ID
        | DRIFT_BRUSHSTROKES_PRESET_ID
        | EDGE_SPARKS_PRESET_ID
        | COLLISION_RIPPLES_PRESET_ID
        | PORTAL_WARP_PRESET_ID
        | DISPLACEMENT_RIPPLE_PRESET_ID
        | REFRACTION_PRESET_ID => TreatmentGroup::SourceModifier,
        _ => TreatmentGroup::GenerativeOrUtility,
    }
}

/// `(preset_id, display_label)` pairs for every preset registered with the
/// renderer. The Selected-layer UI sources its combobox options from this.
///
/// PCleanup.2.12 — source-modifying presets are listed first so the picker
/// can insert a visual separator without sorting. Use [`treatment_group`] to
/// query which group a preset belongs to.
pub fn registry() -> &'static [(&'static str, &'static str)] {
    &[
        // --- Source-modifying treatments (warp / modulate the source photo) ---
        // PCleanup.1.2 — bounded-fluid velocity warp.
        (FLUID_WARP_PRESET_ID, "Fluid warp (velocity field)"),
        // PCleanup.2.3 — unbounded fluid warp; works on any source.
        (FLUID_WARP_FULL_PRESET_ID, "Fluid warp (full)"),
        // PCleanup.2.1 — concentric-ring ripple warp of the source.
        (RIPPLE_LENS_PRESET_ID, "Ripple lens (source warp)"),
        // PCleanup.2.2 — N traveling refraction bumps along the source edge.
        (EDGE_LENS_PRESET_ID, "Edge lens (orbiting refraction)"),
        // PCleanup.2.7 — vector-field advection of the source.
        (FIELD_ADVECT_PRESET_ID, "Field advect (source drift)"),
        // PCleanup.2.9 — luminance boost inside the zone window.
        (ZONE_BRIGHTEN_PRESET_ID, "Zone brighten (luminance boost)"),
        // PCleanup.2.10 — UV lens warp at the zone window edge.
        (ZONE_LENS_PRESET_ID, "Zone lens (source warp at edge)"),
        // PCleanup.2.4 — particle-based luminance boost (Gaussian dot).
        (SPOTLIGHTS_PRESET_ID, "Spotlights (particle luminance lift)"),
        // PCleanup.2.5a — particle-based source mask (drifting peepholes).
        (
            DRIFT_PINHOLES_PRESET_ID,
            "Drift pinholes (particle source mask)",
        ),
        // PCleanup.2.5b — motion-blurred brushstrokes trailing each particle.
        (
            DRIFT_BRUSHSTROKES_PRESET_ID,
            "Drift brushstrokes (motion-blur source mask)",
        ),
        // PCleanup.2.6 — sparks at the mask edge fading over their lifetime.
        (EDGE_SPARKS_PRESET_ID, "Edge sparks (mask-edge embers)"),
        // PCleanup.2.8 — particle collisions on the mask emit ripples.
        (
            COLLISION_RIPPLES_PRESET_ID,
            "Collision ripples (mask-bounce displacement)",
        ),
        // PCleanup.2.11 — drifting particles warp source UVs around them.
        (
            PORTAL_WARP_PRESET_ID,
            "Portal warp (ghost-through-the-room UV smear)",
        ),
        // P2.4.1 — displacement-map ripple warp (pre-W2).
        (DISPLACEMENT_RIPPLE_PRESET_ID, "Displacement ripple"),
        // P2.4.2 — refraction warp (pre-W2).
        (REFRACTION_PRESET_ID, "Refraction"),
        // --- Generative / utility treatments ---
        (IDENTITY_PRESET_ID, "Identity (no-op)"),
        (TONE_MAP_PRESET_ID, "Tone map"),
        (LUMINANCE_REVEAL_PRESET_ID, "Luminance reveal"),
        (BLUR_MASK_PRESET_ID, "Blur mask (edge feather)"),
        (TEXTURE_OVERLAY_PRESET_ID, "Texture overlay"),
        (PALETTE_EXTRACT_PRESET_ID, "Palette / posterize"),
        (COLLAGE_PRESET_ID, "Collage (2×2)"),
    ]
}

/// `true` if `preset_id` corresponds to a registered preset. CPU-only;
/// safe to call without a GPU device (used by audit + tests).
#[allow(dead_code)] // wired by future audit + P1.2.3 UI
pub fn is_registered(preset_id: &str) -> bool {
    registry().iter().any(|(id, _)| *id == preset_id)
}

/// Param descriptors for the named preset. Returns an empty slice for
/// unknown presets and for presets with no tunable parameters (identity).
#[allow(dead_code)] // consumed by `windows::controls` (v3-gated picker)
pub fn param_descriptors(preset_id: &str) -> &'static [ParamDescriptor] {
    match preset_id {
        IDENTITY_PRESET_ID => &[],
        TONE_MAP_PRESET_ID => TONE_MAP_DESCRIPTORS,
        LUMINANCE_REVEAL_PRESET_ID => LUMINANCE_REVEAL_DESCRIPTORS,
        BLUR_MASK_PRESET_ID => BLUR_MASK_DESCRIPTORS,
        TEXTURE_OVERLAY_PRESET_ID => TEXTURE_OVERLAY_DESCRIPTORS,
        PALETTE_EXTRACT_PRESET_ID => PALETTE_EXTRACT_DESCRIPTORS,
        COLLAGE_PRESET_ID => COLLAGE_DESCRIPTORS,
        DISPLACEMENT_RIPPLE_PRESET_ID => DISPLACEMENT_RIPPLE_DESCRIPTORS,
        REFRACTION_PRESET_ID => REFRACTION_DESCRIPTORS,
        // PCleanup.2.1 — first W2 sibling treatment.
        RIPPLE_LENS_PRESET_ID => RIPPLE_LENS_DESCRIPTORS,
        // PCleanup.2.2 — second W2 sibling treatment.
        EDGE_LENS_PRESET_ID => EDGE_LENS_DESCRIPTORS,
        // PCleanup.2.7 — third W2 sibling treatment.
        FIELD_ADVECT_PRESET_ID => FIELD_ADVECT_DESCRIPTORS,
        // PCleanup.1.2 — fluid_warp treatment.
        FLUID_WARP_PRESET_ID => FLUID_WARP_DESCRIPTORS,
        // PCleanup.2.3 — fluid_warp_full treatment.
        FLUID_WARP_FULL_PRESET_ID => FLUID_WARP_FULL_DESCRIPTORS,
        // PCleanup.2.9 — zone_brighten treatment.
        ZONE_BRIGHTEN_PRESET_ID => ZONE_BRIGHTEN_DESCRIPTORS,
        // PCleanup.2.10 — zone_lens treatment.
        ZONE_LENS_PRESET_ID => ZONE_LENS_DESCRIPTORS,
        // PCleanup.2.4 — spotlights treatment.
        SPOTLIGHTS_PRESET_ID => SPOTLIGHTS_DESCRIPTORS,
        // PCleanup.2.5a — drift_pinholes treatment.
        DRIFT_PINHOLES_PRESET_ID => DRIFT_PINHOLES_DESCRIPTORS,
        // PCleanup.2.5b — drift_brushstrokes treatment.
        DRIFT_BRUSHSTROKES_PRESET_ID => DRIFT_BRUSHSTROKES_DESCRIPTORS,
        // PCleanup.2.6 — edge_sparks treatment.
        EDGE_SPARKS_PRESET_ID => EDGE_SPARKS_DESCRIPTORS,
        // PCleanup.2.8 — collision_ripples treatment.
        COLLISION_RIPPLES_PRESET_ID => COLLISION_RIPPLES_DESCRIPTORS,
        // PCleanup.2.11 — portal_warp treatment.
        PORTAL_WARP_PRESET_ID => PORTAL_WARP_DESCRIPTORS,
        _ => &[],
    }
}

/// Static descriptors for the `tone_map` preset's three params.
/// At all defaults (`exposure=0, contrast=1, shoulder=0`) the shader is
/// a passthrough — the preset is visually transparent until the operator
/// tunes a slider.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const TONE_MAP_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "exposure",
        label: "Exposure (stops)",
        min: -2.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "contrast",
        label: "Contrast",
        min: 0.5,
        max: 1.5,
        default: 1.0,
    },
    ParamDescriptor {
        key: "shoulder",
        label: "Highlight rolloff",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
];

/// Static descriptors for the `luminance_reveal` preset (P1.3.3).
/// Defaults: 50 % threshold, gentle softness band, non-inverted.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const LUMINANCE_REVEAL_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "threshold",
        label: "Threshold",
        min: 0.0,
        max: 1.0,
        default: 0.5,
    },
    ParamDescriptor {
        key: "softness",
        label: "Softness",
        min: 0.0,
        max: 0.5,
        default: 0.1,
    },
    ParamDescriptor {
        key: "invert",
        label: "Invert (0/1)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
];

/// Static descriptors for the `collage` preset (P1.3.6).
/// PCleanup.8.3b: added `mode` (0=grid default, 1=kaleidoscope, 2=mosaic).
/// Default mode=0 preserves existing 2×2 grid behaviour for all projects.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const COLLAGE_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "mix",
        label: "Mix (0 = source only, 1 = collage)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "gap",
        label: "Gap (0 = touching, 0.1 = wide seam)",
        min: 0.0,
        max: 0.1,
        default: 0.02,
    },
    ParamDescriptor {
        key: "mode",
        label: "Mode (0=grid, 1=kaleidoscope, 2=mosaic)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
];

/// Static descriptors for the `palette_extract` preset (P1.3.5).
/// PCleanup.8.3a: added `zone_mode` (default 0 = ignore_zone = existing
/// behaviour) and `outside_levels` (only meaningful at zone_mode=2).
/// Default zone_mode=0 preserves pre-8.3a output for all existing projects.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const PALETTE_EXTRACT_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "levels",
        label: "Levels per channel (1-8)",
        min: 1.0,
        max: 8.0,
        default: 4.0,
    },
    ParamDescriptor {
        key: "mix",
        label: "Mix (0 = source, 1 = posterised)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "dither",
        label: "Dither (0 = banded, 1 = noisy)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "zone_mode",
        label: "Zone mode (0=ignore, 1=strict ZONE_WINDOW, 2=dual quantisation)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "outside_levels",
        label: "Levels outside zone (zone_mode=2 only)",
        min: 1.0,
        max: 8.0,
        default: 4.0,
    },
];

/// Static descriptors for the `texture_overlay` preset (P1.3.4).
/// Defaults: mix = 0 (identity passthrough; preset visually transparent
/// until `mix` slides above zero), no offset, Normal blend.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const TEXTURE_OVERLAY_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "mix",
        label: "Mix (0 = source only, 1 = full overlay)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "offset_x",
        label: "Offset X",
        min: -1.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "offset_y",
        label: "Offset Y",
        min: -1.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "blend_mode",
        label: "Blend (0=Normal, 1=Multiply, 2=Screen, 3=Add)",
        min: 0.0,
        max: 3.0,
        default: 1.0,
    },
];

/// Static descriptors for the `blur_mask` preset (P1.3.2).
/// Defaults: zero radius (identity = no blur), 0.1 norm-units edge band
/// (~7 % of layer width), smooth falloff. Identity at default radius =
/// the operator sees no change until they reach for the radius slider.
/// PCleanup.8.3c: added `radius_mode` (0=edge-band default, 1=distance-driven)
/// and `distance_falloff` (only meaningful at radius_mode=1). Default
/// radius_mode=0 preserves existing behaviour exactly for all projects.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const BLUR_MASK_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "max_radius_px",
        label: "Max radius (px)",
        min: 0.0,
        max: 32.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "edge_band",
        label: "Edge band (norm, radius_mode=0 only)",
        min: 0.01,
        max: 0.3,
        default: 0.1,
    },
    ParamDescriptor {
        key: "falloff",
        label: "Falloff (0=hard, 1=smooth, radius_mode=0 only)",
        min: 0.0,
        max: 1.0,
        default: 0.7,
    },
    ParamDescriptor {
        key: "radius_mode",
        label: "Radius mode (0=edge-band, 1=distance-driven)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "distance_falloff",
        label: "Distance falloff (norm, radius_mode=1 only)",
        min: 0.01,
        max: 0.5,
        default: 0.2,
    },
];

/// Static descriptors for the `displacement_ripple` preset (P2.4.1).
/// Identity at default amplitude = 0.0 — the operator sees no change
/// until they increase the amplitude slider. Decay controls how quickly
/// the ripple band falls off from the mask edge; frequency sets the
/// spatial frequency of the sinusoidal modulation.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const DISPLACEMENT_RIPPLE_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "amplitude",
        label: "Amplitude (UV units)",
        min: 0.0,
        max: 0.05,
        default: 0.0,
    },
    ParamDescriptor {
        key: "frequency",
        label: "Frequency (ripples/unit)",
        min: 1.0,
        max: 20.0,
        default: 8.0,
    },
    ParamDescriptor {
        key: "decay",
        label: "Decay (0=narrow band, 1=wide)",
        min: 0.0,
        max: 1.0,
        default: 0.5,
    },
];

/// PCleanup.2.2 — Static descriptors for the `edge_lens` treatment.
/// Identity at default `amplitude = 0.0`. `n_waves` controls the crest
/// count orbiting the boundary (clamped to integer in the shader);
/// `speed` drives the angular travel rate (clock-driven, populated
/// each frame by the dispatcher into the params uniform's `w` slot).
#[allow(dead_code)] // referenced through `param_descriptors`
const EDGE_LENS_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "amplitude",
        label: "Amplitude (UV displacement)",
        min: 0.0,
        max: 0.1,
        default: 0.0,
    },
    ParamDescriptor {
        key: "n_waves",
        label: "Crests around boundary (1–8)",
        min: 1.0,
        max: 8.0,
        default: 4.0,
    },
    ParamDescriptor {
        key: "speed",
        label: "Animation speed (cycles/sec)",
        min: 0.0,
        max: 5.0,
        default: 1.0,
    },
];

/// PCleanup.2.1 — Static descriptors for the `ripple_lens` treatment.
/// Identity at default `amplitude = 0.0` — the operator sees no change
/// until they increase the slider. `wavelength` controls how tightly
/// the concentric rings are spaced; `speed` is reserved for a future
/// clock-driven animation pass (currently inert; rings are static).
#[allow(dead_code)] // referenced through `param_descriptors`
const RIPPLE_LENS_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "amplitude",
        label: "Amplitude (UV displacement)",
        min: 0.0,
        max: 0.1,
        default: 0.0,
    },
    ParamDescriptor {
        key: "wavelength",
        label: "Wavelength (distance between rings)",
        min: 0.01,
        max: 0.5,
        default: 0.08,
    },
    ParamDescriptor {
        key: "speed",
        label: "Animation speed (cycles/sec; reserved)",
        min: 0.0,
        max: 5.0,
        default: 0.0,
    },
];

/// PCleanup.2.7 — Static descriptors for the `field_advect_source` treatment.
/// Identity at default `flow_speed = 0.0` — adding this treatment without
/// configuring it is a guaranteed no-op (offset = gradient × 0 × clock = 0).
/// Higher `flow_speed` drifts the photo along mask normals at increasing rate.
#[allow(dead_code)] // referenced through `param_descriptors`
const FIELD_ADVECT_DESCRIPTORS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "flow_speed",
    label: "Flow speed (UV/s along gradient)",
    min: 0.0,
    max: 2.0,
    default: 0.0,
}];

/// PCleanup.1.2 — Static descriptors for the `fluid_warp` treatment.
/// Identity at default `amplitude = 0.0` — adding this treatment without
/// configuring it is a guaranteed no-op (offset = vel × 0 = vec2(0)).
/// Higher `amplitude` scales the displacement of the velocity field.
#[allow(dead_code)] // referenced through `param_descriptors`
const FLUID_WARP_DESCRIPTORS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "amplitude",
    label: "Warp amplitude (UV displacement scale)",
    min: 0.0,
    max: 2.0,
    default: 0.0,
}];

/// PCleanup.2.3 — Static descriptors for the `fluid_warp_full` treatment.
/// Identity at default `amplitude = 0.0` — adding this treatment without
/// configuring it is a guaranteed no-op (offset = vel × 0 = vec2(0)).
/// Higher `amplitude` scales the displacement of the full-layer velocity field.
#[allow(dead_code)] // referenced through `param_descriptors`
const FLUID_WARP_FULL_DESCRIPTORS: &[ParamDescriptor] = &[ParamDescriptor {
    key: "amplitude",
    label: "Warp amplitude (UV displacement scale)",
    min: 0.0,
    max: 2.0,
    default: 0.0,
}];

/// PCleanup.2.9 — Static descriptors for the `zone_brighten` treatment.
/// Identity at default `intensity = 0.0` — the multiplier is `1.0 + 0.0 *
/// exp(…) = 1.0` everywhere, so adding the treatment is a guaranteed no-op
/// until the operator configures it (mirrors the established identity-default
/// rule for all W2 sibling treatments). `falloff` and `spill_radius` are
/// shape params matching the fx_zone_light_spill FX sibling's geometry.
/// `speed` drives the optional breathing pulse (0 = constant, no pulse).
#[allow(dead_code)] // referenced through `param_descriptors`
const ZONE_BRIGHTEN_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "intensity",
        label: "Brightness boost (0 = no effect)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "falloff",
        label: "Falloff sharpness (higher = narrower band)",
        min: 0.0,
        max: 20.0,
        default: 8.0,
    },
    ParamDescriptor {
        key: "spill_radius",
        label: "Reach inside zone (normalised)",
        min: 0.0,
        max: 1.0,
        default: 0.3,
    },
    ParamDescriptor {
        key: "speed",
        label: "Breathing pulse rate (cycles/sec; 0 = constant)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
];

/// PCleanup.2.10 — Static descriptors for the `zone_lens` treatment.
/// Identity at default `amplitude = 0.0` — displacement vector is
/// `normal * sin(...) * amplitude * band_weight` which is zero everywhere
/// when amplitude = 0, so the output is bit-identical to the source (mirrors
/// the established identity-default rule for all W2 sibling treatments).
/// `band_width` controls the exponential decay constant; `frequency` and
/// `speed` control the spatial and temporal animation of the sine ripple.
#[allow(dead_code)] // referenced through `param_descriptors`
const ZONE_LENS_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "amplitude",
        label: "Lens amplitude (0 = no effect)",
        min: 0.0,
        max: 0.05,
        default: 0.0,
    },
    ParamDescriptor {
        key: "speed",
        label: "Animation speed (cycles/sec)",
        min: 0.0,
        max: 3.0,
        default: 1.0,
    },
    ParamDescriptor {
        key: "band_width",
        label: "Edge band width (exp decay constant)",
        min: 0.0,
        max: 0.3,
        default: 0.05,
    },
    ParamDescriptor {
        key: "frequency",
        label: "Ripple frequency along band",
        min: 0.0,
        max: 40.0,
        default: 10.0,
    },
];

/// PCleanup.2.4 — Static descriptors for the `spotlights` preset.
///
/// Identity at `brightness_gain = 0.0`: gain=0 → weight × 0 = 0 lift →
/// multiplier = 1.0 everywhere → source passes through unchanged.
///
/// Particle count: 1..=512; default 32 (per the spec).
/// Radius: normalised UV radius of each spotlight's Gaussian influence area.
/// Drift speed: UV/s; 0 = static particles (useful for spatial accents).
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const SPOTLIGHTS_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Spotlight count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 32.0,
    },
    ParamDescriptor {
        key: "brightness_gain",
        label: "Brightness gain (0 = no effect)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "radius",
        label: "Spotlight radius (normalised UV)",
        min: 0.01,
        max: 0.3,
        default: 0.05,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Drift speed (UV/s)",
        min: 0.0,
        max: 1.0,
        default: 0.1,
    },
];

/// PCleanup.2.5a — Static descriptors for the `drift_pinholes` preset.
///
/// Identity at `opacity = 0.0`: the fragment `mix(src, masked, 0.0)` collapses
/// to `src` regardless of particle positions, so a freshly-added Treatment is
/// a bit-exact passthrough until the operator pulls the opacity slider up.
///
/// Shares `particle_count`, `radius`, `drift_speed` semantics with
/// `spotlights` — same compute pass, same SSBO, same MAX_SPOTLIGHTS cap.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const DRIFT_PINHOLES_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Pinhole count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 32.0,
    },
    ParamDescriptor {
        key: "opacity",
        label: "Pinhole opacity (0 = no effect, 1 = fully masked)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "radius",
        label: "Pinhole radius (normalised UV)",
        min: 0.01,
        max: 0.3,
        default: 0.05,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Drift speed (UV/s)",
        min: 0.0,
        max: 1.0,
        default: 0.1,
    },
];

/// PCleanup.2.5b — Static descriptors for the `drift_brushstrokes` preset.
///
/// Identity at `opacity = 0.0`: the fragment `mix(src, brush, 0.0)` collapses
/// to `src` regardless of velocity vectors, so a freshly-added Treatment is
/// a bit-exact passthrough until the operator pulls opacity up.
///
/// `smear_duration` controls how many seconds of motion the brushstroke
/// trails behind each particle.  At `drift_speed = 0` (no motion), the
/// stroke degrades to a circular Gaussian matching drift_pinholes.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const DRIFT_BRUSHSTROKES_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Brush count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 32.0,
    },
    ParamDescriptor {
        key: "opacity",
        label: "Brush opacity (0 = no effect, 1 = fully masked)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "radius",
        label: "Brush thickness (normalised UV)",
        min: 0.01,
        max: 0.3,
        default: 0.05,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Drift speed (UV/s) — drives brush direction and trail length",
        min: 0.0,
        max: 1.0,
        default: 0.3,
    },
    ParamDescriptor {
        key: "smear_duration",
        label: "Trail length (seconds of motion behind each brush)",
        min: 0.0,
        max: 2.0,
        default: 0.5,
    },
];

/// PCleanup.2.6 — Static descriptors for the `edge_sparks` preset.
///
/// Identity at `brightness_gain = 0.0`: the additive multiplier collapses
/// to `1.0` everywhere → source unchanged. Sparks spawn at the mask edge,
/// drift outward along the SDF gradient over `lifetime_s` seconds, and
/// brighten source pixels they pass over by a Gaussian falloff scaled by
/// remaining life-fraction.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const EDGE_SPARKS_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Spark count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 64.0,
    },
    ParamDescriptor {
        key: "brightness_gain",
        label: "Spark brightness (0 = no effect)",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "radius",
        label: "Spark glow radius (normalised UV)",
        min: 0.01,
        max: 0.3,
        default: 0.04,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Outward drift speed (UV/s along SDF normal)",
        min: 0.0,
        max: 1.0,
        default: 0.15,
    },
    ParamDescriptor {
        key: "lifetime_s",
        label: "Spark lifetime (seconds before respawn)",
        min: 0.1,
        max: 4.0,
        default: 1.5,
    },
];

/// PCleanup.2.8 — Static descriptors for the `collision_ripples` preset.
///
/// Identity at `amplitude = 0.0`: total displacement is zero everywhere, so
/// the fragment samples `t_source` at the original UV — bit-exact pass.
/// `frequency` and `ripple_speed` control the visual shape; `ripple_decay`
/// tunes how fast each bounce's ripple fades.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const COLLISION_RIPPLES_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Bouncer count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 64.0,
    },
    ParamDescriptor {
        key: "amplitude",
        label: "Ripple amplitude (UV displacement; 0 = no effect)",
        min: 0.0,
        max: 0.05,
        default: 0.0,
    },
    ParamDescriptor {
        key: "frequency",
        label: "Ripple frequency (higher = tighter ring)",
        min: 1.0,
        max: 80.0,
        default: 20.0,
    },
    ParamDescriptor {
        key: "ripple_speed",
        label: "Ring expansion speed (UV/s)",
        min: 0.0,
        max: 2.0,
        default: 0.5,
    },
    ParamDescriptor {
        key: "ripple_decay",
        label: "Ripple decay (1/s; higher = fades sooner)",
        min: 0.0,
        max: 5.0,
        default: 1.0,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Bouncer drift speed (UV/s)",
        min: 0.0,
        max: 1.0,
        default: 0.3,
    },
    ParamDescriptor {
        key: "ripple_lifetime",
        label: "Ripple lifetime (seconds before respawn)",
        min: 0.1,
        max: 4.0,
        default: 1.2,
    },
];

/// PCleanup.2.11 — Static descriptors for the `portal_warp` preset.
///
/// Identity at `amplitude = 0.0`: accumulated displacement is zero, source
/// samples at original UV.  `pull` selects pull-toward (+1) vs push-away
/// (-1) and intermediate blend.  Same compute as spotlights, so the
/// particle-count / drift_speed semantics match.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const PORTAL_WARP_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "particle_count",
        label: "Ghost count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 32.0,
    },
    ParamDescriptor {
        key: "amplitude",
        label: "Warp amplitude (UV displacement; 0 = no effect)",
        min: 0.0,
        max: 0.05,
        default: 0.0,
    },
    ParamDescriptor {
        key: "radius",
        label: "Warp falloff radius (normalised UV)",
        min: 0.01,
        max: 0.3,
        default: 0.1,
    },
    ParamDescriptor {
        key: "pull",
        label: "Pull (+1) vs push (-1) direction",
        min: -1.0,
        max: 1.0,
        default: 1.0,
    },
    ParamDescriptor {
        key: "drift_speed",
        label: "Ghost drift speed (UV/s)",
        min: 0.0,
        max: 1.0,
        default: 0.2,
    },
];

/// Static descriptors for the `refraction` preset (P2.4.2).
/// Identity at default ior = 1.0 — the operator sees no change until they
/// increase the ior slider. edge_width controls the SDF-distance band around
/// the mask edge where refraction applies.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const REFRACTION_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "ior",
        label: "Index of refraction (1.0 = none)",
        min: 1.0,
        max: 2.0,
        default: 1.0,
    },
    ParamDescriptor {
        key: "edge_width",
        label: "Edge band width (normalised SDF distance)",
        min: 0.0,
        max: 0.3,
        default: 0.1,
    },
];

/// Per-preset render pipelines. One field per preset; dispatch is a `match`
/// on `preset_id`. Mirrors the `FxPresetPipeline` shape so adding a preset
/// is "add a field + add a match arm" with no trait-object dispatch.
pub struct TreatmentPipeline {
    identity: IdentityTreatmentPipeline,
    tone_map: ToneMapTreatmentPipeline,
    luminance_reveal: LuminanceRevealTreatmentPipeline,
    blur_mask: BlurMaskTreatmentPipeline,
    texture_overlay: TextureOverlayTreatmentPipeline,
    palette_extract: PaletteExtractTreatmentPipeline,
    collage: CollageTreatmentPipeline,
    displacement_ripple: DisplacementRippleTreatmentPipeline,
    // PCleanup.2.1 — first W2 sibling treatment (SourceModifier as Treatment).
    ripple_lens: RippleLensTreatmentPipeline,
    // PCleanup.2.2 — second W2 sibling treatment.
    edge_lens: EdgeLensTreatmentPipeline,
    // PCleanup.2.7 — third W2 sibling treatment.
    field_advect: FieldAdvectTreatmentPipeline,
    refraction: RefractionTreatmentPipeline,
    // PCleanup.1.2 — fluid_warp (bounded-fluid velocity warp).
    fluid_warp: FluidWarpTreatmentPipeline,
    // PCleanup.2.3 — fluid_warp_full (unbounded; identity compute; no SDF).
    fluid_warp_full: FluidWarpFullTreatmentPipeline,
    // PCleanup.2.9 — fifth W2 sibling: luminance boost inside ZONE_WINDOW.
    zone_brighten: ZoneBrightenTreatmentPipeline,
    // PCleanup.2.10 — sixth W2 sibling: UV lens warp at ZONE_WINDOW edge.
    zone_lens: ZoneLensTreatmentPipeline,
    // PCleanup.2.4 — seventh W2 sibling: particle-based luminance boost.
    spotlights: crate::render::treatment_particles::TreatmentParticlePipeline,
    // PCleanup.2.5a — eighth W2 sibling: particle-based source mask.
    drift_pinholes: crate::render::treatment_particles::TreatmentParticlePipeline,
    // PCleanup.2.5b — ninth W2 sibling: motion-blurred brushstrokes (reads vel).
    drift_brushstrokes: crate::render::treatment_particles::TreatmentParticlePipeline,
    // PCleanup.2.6 — tenth W2 sibling: edge-spawning sparks with lifetime fade.
    edge_sparks: crate::render::treatment_particles::TreatmentParticlePipeline,
    // PCleanup.2.8 — eleventh W2 sibling: particle bounces emit UV-displacing
    // ripples (GPU-only, no CPU readback — state encoded in Particle._pad).
    collision_ripples: crate::render::treatment_particles::TreatmentParticlePipeline,
    // PCleanup.2.11 — twelfth W2 sibling: drifting particles smear source UVs.
    portal_warp: crate::render::treatment_particles::TreatmentParticlePipeline,
}

impl TreatmentPipeline {
    /// Build every preset's pipeline against `target_format` (the effect
    /// chain's ping-pong format — same as the surface format).
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            identity: IdentityTreatmentPipeline::new(device, target_format),
            tone_map: ToneMapTreatmentPipeline::new(device, target_format),
            luminance_reveal: LuminanceRevealTreatmentPipeline::new(device, target_format),
            blur_mask: BlurMaskTreatmentPipeline::new(device, target_format),
            texture_overlay: TextureOverlayTreatmentPipeline::new(device, target_format),
            palette_extract: PaletteExtractTreatmentPipeline::new(device, target_format),
            collage: CollageTreatmentPipeline::new(device, target_format),
            displacement_ripple: DisplacementRippleTreatmentPipeline::new(device, target_format),
            // PCleanup.2.1 — first W2 sibling treatment.
            ripple_lens: RippleLensTreatmentPipeline::new(device, target_format),
            // PCleanup.2.2 — second W2 sibling treatment.
            edge_lens: EdgeLensTreatmentPipeline::new(device, target_format),
            // PCleanup.2.7 — third W2 sibling treatment.
            field_advect: FieldAdvectTreatmentPipeline::new(device, target_format),
            refraction: RefractionTreatmentPipeline::new(device, target_format),
            // PCleanup.1.2 — fluid_warp.
            fluid_warp: FluidWarpTreatmentPipeline::new(device, target_format),
            // PCleanup.2.3 — fluid_warp_full.
            fluid_warp_full: FluidWarpFullTreatmentPipeline::new(device, target_format),
            // PCleanup.2.9 — zone_brighten.
            zone_brighten: ZoneBrightenTreatmentPipeline::new(device, target_format),
            // PCleanup.2.10 — zone_lens.
            zone_lens: ZoneLensTreatmentPipeline::new(device, target_format),
            // PCleanup.2.4 — spotlights particle pipeline.
            spotlights:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_spotlights(
                    device,
                    target_format,
                ),
            // PCleanup.2.5a — drift_pinholes particle pipeline (shares compute,
            // different fragment shader from spotlights).
            drift_pinholes:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_drift_pinholes(
                    device,
                    target_format,
                ),
            // PCleanup.2.5b — drift_brushstrokes particle pipeline (shares
            // compute pass; fragment shader reads particle velocity).
            drift_brushstrokes:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_drift_brushstrokes(
                    device,
                    target_format,
                ),
            // PCleanup.2.6 — edge_sparks (different compute shader: spawns
            // particles at the mask edge and tracks per-particle lifetime).
            edge_sparks:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_edge_sparks(
                    device,
                    target_format,
                ),
            // PCleanup.2.8 — collision_ripples (different compute: drift+collide
            // state machine; fragment displaces UVs by active ripple sum).
            collision_ripples:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_collision_ripples(
                    device,
                    target_format,
                ),
            // PCleanup.2.11 — portal_warp (shared spotlights compute; fragment
            // smears UVs toward / away from each particle).
            portal_warp:
                crate::render::treatment_particles::TreatmentParticlePipeline::new_portal_warp(
                    device,
                    target_format,
                ),
        }
    }

    /// Render `inputs` through the preset named by `preset_id` into `dst`.
    ///
    /// Returns `true` if the dispatch ran. Returns `false` for an
    /// unregistered `preset_id`, leaving `dst` untouched — the caller is
    /// expected to fall back to the default `svg_pipeline` blit so the
    /// layer's source content still appears.
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        preset_id: &str,
    ) -> bool {
        match preset_id {
            IDENTITY_PRESET_ID => {
                self.identity.render(device, encoder, dst, inputs);
                true
            }
            TONE_MAP_PRESET_ID => {
                self.tone_map.render(device, queue, encoder, dst, inputs);
                true
            }
            LUMINANCE_REVEAL_PRESET_ID => {
                self.luminance_reveal
                    .render(device, queue, encoder, dst, inputs);
                true
            }
            BLUR_MASK_PRESET_ID => {
                // blur_mask is multi-pass and consumes the layer's SDF +
                // scratch texture. If either is missing (caller did not
                // populate them, e.g. SVG/FxLayer route), skip the dispatch
                // and let the caller's fallback render the source unblurred.
                let (Some(sdf), Some(intermediate)) = (inputs.sdf, inputs.intermediate) else {
                    return false;
                };
                self.blur_mask
                    .render(device, queue, encoder, dst, inputs, sdf, intermediate);
                true
            }
            TEXTURE_OVERLAY_PRESET_ID => {
                // texture_overlay needs `inputs.overlay` populated by the
                // caller (loaded from `Treatment.overlay_path` via the
                // ImageTextureCache). Missing overlay → skip + caller
                // falls back to the default blit, so a half-configured
                // treatment renders the source unaltered.
                let Some(overlay) = inputs.overlay else {
                    return false;
                };
                self.texture_overlay
                    .render(device, queue, encoder, dst, inputs, overlay);
                true
            }
            PALETTE_EXTRACT_PRESET_ID => {
                self.palette_extract
                    .render(device, queue, encoder, dst, inputs);
                true
            }
            COLLAGE_PRESET_ID => {
                // collage always renders — empty slots fall back to
                // source inside the shader (slot_mask bit cleared). At
                // mix=0 the operator sees pure source even with 4
                // slots populated, so we never refuse the dispatch.
                self.collage.render(device, queue, encoder, dst, inputs);
                true
            }
            DISPLACEMENT_RIPPLE_PRESET_ID => {
                // displacement_ripple requires the layer SDF. If missing
                // (e.g. SVG / FxLayer route), skip the dispatch and let
                // the caller's fallback render the source unaltered.
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.displacement_ripple
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.1 — first W2 sibling treatment. Same SDF
            // requirement as displacement_ripple — skip when the layer
            // route doesn't provide one.
            RIPPLE_LENS_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.ripple_lens
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.2 — second W2 sibling treatment. Like ripple_lens,
            // skips when no SDF is present (SVG / FxLayer routes).
            EDGE_LENS_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.edge_lens
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.7 — third W2 sibling treatment. Advects the source
            // image along the SDF gradient field. Skips when no SDF present.
            FIELD_ADVECT_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.field_advect
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            REFRACTION_PRESET_ID => {
                // refraction requires the layer SDF. If missing
                // (e.g. SVG / FxLayer route), skip the dispatch and let
                // the caller's fallback render the source unaltered.
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.refraction
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.1.2 — fluid_warp. Requires layer SDF for the compute
            // boundary pass; skips when SDF is absent (SVG / FxLayer routes).
            FLUID_WARP_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.fluid_warp
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.3 — fluid_warp_full. Uses fluid_identity (no SDF);
            // works on any layer source — no SDF guard needed.
            FLUID_WARP_FULL_PRESET_ID => {
                self.fluid_warp_full
                    .render(device, queue, encoder, dst, inputs);
                true
            }
            // PCleanup.2.9 — zone_brighten. Requires SDF (distance-to-edge
            // drives the brightness falloff). Without a zone_role of
            // ZONE_WINDOW the shader passes source through unchanged; the
            // dispatch still runs so the caller sees `true` and doesn't
            // fall back to a blank blit. Skip when no SDF is present.
            ZONE_BRIGHTEN_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.zone_brighten
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.10 — zone_lens. Requires SDF (edge-band shape is
            // driven by unsigned distance-to-edge). Without a zone_role of
            // ZONE_WINDOW the shader passes source through unchanged; the
            // dispatch still runs so the caller sees `true` and doesn't
            // fall back to a blank blit. Skip when no SDF is present.
            ZONE_LENS_PRESET_ID => {
                let Some(sdf) = inputs.sdf else {
                    return false;
                };
                self.zone_lens
                    .render(device, queue, encoder, dst, inputs, sdf);
                true
            }
            // PCleanup.2.4 — spotlights. Two-pass: compute (particle
            // position-update) then fragment (Gaussian luminance boost).
            // SDF is optional: `None` → particles spawn uniformly in [0,1]²
            // (no mask constraint). Dispatches even without SDF so the preset
            // remains useful on layers that have no mask.
            SPOTLIGHTS_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(32.0) as u32;
                self.spotlights.dispatch_compute(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.spotlights.render(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                );
                true
            }
            // PCleanup.2.5a — drift_pinholes. Same compute pass as spotlights;
            // the fragment masks the source by particle proximity instead of
            // lifting luminance.  SDF is optional (same fallback as spotlights).
            DRIFT_PINHOLES_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(32.0) as u32;
                self.drift_pinholes.dispatch_compute(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.drift_pinholes.render_drift_pinholes(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                );
                true
            }
            // PCleanup.2.5b — drift_brushstrokes. Same compute pass (now
            // writing per-particle vel); fragment shader reads vel and draws
            // elongated motion-blur strokes trailing each particle.
            DRIFT_BRUSHSTROKES_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(32.0) as u32;
                self.drift_brushstrokes.dispatch_compute(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.drift_brushstrokes.render_drift_brushstrokes(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                );
                true
            }
            // PCleanup.2.6 — edge_sparks. Different compute (spawns at mask
            // edge, tracks per-particle lifetime). Fragment fades each spark
            // over its lifetime.
            EDGE_SPARKS_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(64.0) as u32;
                self.edge_sparks.dispatch_compute_edge_sparks(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.edge_sparks.render_edge_sparks(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                    inputs.clock_secs - inputs.t_layer_added_secs,
                );
                true
            }
            // PCleanup.2.8 — collision_ripples. Drift+collide state machine
            // in compute; fragment displaces source UVs by accumulated radial
            // ripples from each active collision.
            COLLISION_RIPPLES_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(64.0) as u32;
                self.collision_ripples.dispatch_compute_collision_ripples(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.collision_ripples.render_collision_ripples(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                    inputs.clock_secs - inputs.t_layer_added_secs,
                );
                true
            }
            // PCleanup.2.11 — portal_warp. Same compute pass as spotlights
            // (drift in mask); fragment smears source UVs around each particle.
            PORTAL_WARP_PRESET_ID => {
                let n = inputs.params.get("particle_count").copied().unwrap_or(32.0) as u32;
                self.portal_warp.dispatch_compute(
                    queue,
                    device,
                    encoder,
                    n,
                    inputs.seed,
                    inputs.clock_secs,
                    inputs.t_layer_added_secs,
                    inputs.params,
                    inputs.sdf,
                );
                self.portal_warp.render_portal_warp(
                    device,
                    queue,
                    encoder,
                    dst,
                    inputs.source,
                    inputs.params,
                    n,
                );
                true
            }
            _ => false,
        }
    }
}

/// Identity treatment: blits `source` → `dst` through the same
/// `textured_quad.wgsl` shader the default path uses. Owns its own
/// `wgpu::RenderPipeline` (intentionally not shared with `svg_pipeline`)
/// so an end-to-end "identity treatment matches default path" test
/// exercises the dispatch code path rather than aliasing it.
struct IdentityTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl IdentityTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treatment identity (textured_quad.wgsl)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/textured_quad.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treatment identity bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treatment identity pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treatment identity pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treatment identity sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treatment identity bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treatment identity pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Tone-map treatment pipeline (P1.3.1). Reads exposure / contrast /
/// shoulder from `inputs.params`, applies the S-curve to each sampled
/// fragment, and writes into `dst` with alpha preserved.
struct ToneMapTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl ToneMapTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_tone_map.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/treat_tone_map.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treatment tone_map bgl"),
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
                // fit_uniform (16 bytes)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // tone_map params (16 bytes)
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treatment tone_map pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treatment tone_map pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treatment tone_map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treatment tone_map params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        // Resolve params: exposure / contrast / shoulder fall back to the
        // descriptor defaults documented in `TONE_MAP_DESCRIPTORS`. The
        // identity defaults make the shader bit-exact passthrough.
        let exposure = inputs.params.get("exposure").copied().unwrap_or(0.0);
        let contrast = inputs.params.get("contrast").copied().unwrap_or(1.0);
        let shoulder = inputs.params.get("shoulder").copied().unwrap_or(0.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&exposure.to_le_bytes());
        bytes[4..8].copy_from_slice(&contrast.to_le_bytes());
        bytes[8..12].copy_from_slice(&shoulder.to_le_bytes());
        // bytes[12..16] reserved (zero).
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treatment tone_map bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treatment tone_map pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Luminance-reveal treatment pipeline (P1.3.3). Threshold + softness +
/// invert against Rec. 601 luma; RGB passes through with premultiplied
/// alpha modulation. Pipeline shape mirrors `ToneMapTreatmentPipeline`
/// exactly — the only difference is the shader source — so adding W3's
/// remaining single-pass presets follows this same template.
struct LuminanceRevealTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl LuminanceRevealTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let (pipeline, bind_group_layout, sampler, params_buf) = build_single_pass_treatment(
            device,
            target_format,
            "treat_luminance_reveal.wgsl",
            include_str!("shaders/treat_luminance_reveal.wgsl"),
        );
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let threshold = inputs.params.get("threshold").copied().unwrap_or(0.5);
        let softness = inputs.params.get("softness").copied().unwrap_or(0.1);
        let invert = inputs.params.get("invert").copied().unwrap_or(0.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&threshold.to_le_bytes());
        bytes[4..8].copy_from_slice(&softness.to_le_bytes());
        bytes[8..12].copy_from_slice(&invert.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        draw_single_pass_treatment(
            device,
            encoder,
            dst,
            inputs,
            &self.pipeline,
            &self.bind_group_layout,
            &self.sampler,
            &self.params_buf,
            "treatment luminance_reveal",
        );
    }
}

/// Blur-mask treatment pipeline (P1.3.2). Multi-pass:
///   1. Fit pass: source → dst (apply cover/contain crop into ping-pong
///      first slot).
///   2. H pass: dst → intermediate (horizontal gaussian, SDF-gated
///      radius).
///   3. V pass: intermediate → dst (vertical gaussian, same SDF math).
///
/// Identity at default params: `max_radius_px = 0` → blur radius is
/// zero everywhere → V pass output equals the fit-only output, which
/// matches the no-treatment default path. So the preset is visually
/// transparent until the operator increases the radius slider.
struct BlurMaskTreatmentPipeline {
    // Fit pass (textured_quad.wgsl) — same shape as IdentityTreatmentPipeline
    // but owned separately to keep BlurMask's pipeline state isolated.
    fit_pipeline: wgpu::RenderPipeline,
    fit_bgl: wgpu::BindGroupLayout,
    fit_sampler: wgpu::Sampler,

    // H + V passes — same shader-pair shape as the existing BlurPipeline,
    // augmented with an SDF binding for per-fragment radius gating.
    blur_h_pipeline: wgpu::RenderPipeline,
    blur_v_pipeline: wgpu::RenderPipeline,
    blur_bgl: wgpu::BindGroupLayout,
    blur_sampler: wgpu::Sampler,
    blur_sdf_sampler: wgpu::Sampler,
    blur_params_buf: wgpu::Buffer,
}

impl BlurMaskTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // ---- Fit pass (textured_quad.wgsl, ALPHA_BLENDING) -----------
        let fit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask fit (textured_quad.wgsl)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/textured_quad.wgsl").into()),
        });

        let fit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_blur_mask fit bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let fit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_blur_mask fit pipeline layout"),
            bind_group_layouts: &[Some(&fit_bgl)],
            immediate_size: 0,
        });

        let fit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask fit pipeline"),
            layout: Some(&fit_layout),
            vertex: wgpu::VertexState {
                module: &fit_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &fit_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let fit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask fit sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // ---- H + V passes (treat_blur_mask_{h,v}.wgsl) ---------------
        // Both shaders share the same bind layout: source, sampler,
        // params, SDF. build.rs concatenates sdf_helper.wgsl at the
        // front (SDF_CONSUMERS prefix match "treat_blur").
        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_blur_mask blur bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // PCleanup.8.3c: expanded from 16 to 32 bytes
                        // (array<vec4<f32>, 2>) for radius_mode + distance_falloff.
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                // SDF texture — R32Float, unfilterable. The helper uses
                // textureLoad, so the sampler slot is unused at runtime
                // but the shader declaration requires a binding type.
                // Keep the texture as the only entry to keep the bind
                // group small.
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });

        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_blur_mask blur pipeline layout"),
            bind_group_layouts: &[Some(&blur_bgl)],
            immediate_size: 0,
        });

        // Both shaders need the SDF helper prepended at runtime to match
        // build.rs's compile-time validation (which already prepended
        // it via the SDF_CONSUMERS rule).
        let h_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_blur_mask_h.wgsl")
        );
        let v_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_blur_mask_v.wgsl")
        );
        let h_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask_h.wgsl"),
            source: wgpu::ShaderSource::Wgsl(h_src.into()),
        });
        let v_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask_v.wgsl"),
            source: wgpu::ShaderSource::Wgsl(v_src.into()),
        });

        let blur_h_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask H pipeline"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &h_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &h_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // H pass: REPLACE so the intermediate texture is
                    // fully populated each frame (no read-back risk).
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blur_v_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask V pipeline"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &v_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &v_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // V pass: ALPHA_BLENDING so it integrates with the
                    // standard pre-effect ping-pong contract.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask blur sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // NonFiltering sampler for the SDF (R32Float). textureLoad
        // doesn't use the sampler, but binding type requires one.
        let blur_sdf_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask sdf sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // PCleanup.8.3c: expanded to 32 bytes (array<vec4<f32>, 2>) to
        // accommodate the new `radius_mode` and `distance_falloff` params.
        let blur_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_blur_mask params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            fit_pipeline,
            fit_bgl,
            fit_sampler,
            blur_h_pipeline,
            blur_v_pipeline,
            blur_bgl,
            blur_sampler,
            blur_sdf_sampler,
            blur_params_buf,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
        intermediate: &wgpu::TextureView,
    ) {
        // Read params (operator-tuned values, falling back to descriptor
        // defaults). Pack into 32-byte uniform: array<vec4<f32>, 2>.
        // vec4[0]: [max_radius_px, edge_band, falloff, radius_mode]
        // vec4[1]: [distance_falloff, _pad, _pad, _pad]
        // PCleanup.8.3c: radius_mode (default 0 = edge-band, existing behaviour);
        // distance_falloff only meaningful at radius_mode=1.
        let max_radius = inputs.params.get("max_radius_px").copied().unwrap_or(0.0);
        let edge_band = inputs.params.get("edge_band").copied().unwrap_or(0.1);
        let falloff = inputs.params.get("falloff").copied().unwrap_or(0.7);
        let radius_mode = inputs.params.get("radius_mode").copied().unwrap_or(0.0);
        let distance_falloff = inputs
            .params
            .get("distance_falloff")
            .copied()
            .unwrap_or(0.2);
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&max_radius.to_le_bytes());
        bytes[4..8].copy_from_slice(&edge_band.to_le_bytes());
        bytes[8..12].copy_from_slice(&falloff.to_le_bytes());
        bytes[12..16].copy_from_slice(&radius_mode.to_le_bytes());
        bytes[16..20].copy_from_slice(&distance_falloff.to_le_bytes());
        // bytes[20..32] — padding, left as zero.
        queue.write_buffer(&self.blur_params_buf, 0, &bytes);

        // ---- Pass 1: fit → dst (textured_quad with cover/contain) -----
        let fit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_blur_mask fit bg"),
            layout: &self.fit_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.fit_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("treat_blur_mask fit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.fit_pipeline);
            pass.set_bind_group(0, &fit_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        // ---- Pass 2: H blur → intermediate ----
        self.run_blur_pass(
            device,
            encoder,
            "treat_blur_mask H pass",
            &self.blur_h_pipeline,
            dst,          // source: dst (now holds fit-applied content)
            intermediate, // target: scratch
            sdf,
            // H pass into intermediate uses LoadOp::Clear so the previous
            // frame's scratch never bleeds in.
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );

        // ---- Pass 3: V blur → dst (overwrites the fit-applied content) ----
        self.run_blur_pass(
            device,
            encoder,
            "treat_blur_mask V pass",
            &self.blur_v_pipeline,
            intermediate, // source: scratch (H pass result)
            dst,          // target: ping-pong first slot
            sdf,
            // V pass overwrites the fit-applied content with the
            // fully-blurred result; LoadOp::Clear is correct.
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn run_blur_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        sdf: &wgpu::TextureView,
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.blur_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });
        // The SDF NonFiltering sampler is unused at runtime (textureLoad
        // bypasses samplers) but kept so future presets that DO want a
        // filtered SDF sample have the slot wired. Touch to suppress
        // unused-field warnings without changing layout.
        let _ = &self.blur_sdf_sampler;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Build a single-pass treatment pipeline (one render pipeline, fit
/// uniform at binding 2, params uniform at binding 3, ALPHA_BLENDING).
/// Used by every W3 preset that's a simple sample → transform → emit
/// fullscreen pass — tone_map, luminance_reveal, and the equivalent
/// shape preset that follows. Heavier presets (blur_mask is two-pass,
/// collage takes multiple inputs) will get their own constructors.
#[allow(clippy::type_complexity)]
fn build_single_pass_treatment(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader_label: &'static str,
    shader_src: &'static str,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
    wgpu::Sampler,
    wgpu::Buffer,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(shader_label),
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
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(16),
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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(shader_label),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(shader_label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(shader_label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(shader_label),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    (pipeline, bind_group_layout, sampler, params_buf)
}

/// Execute one single-pass treatment draw — fresh bind group per call,
/// `LoadOp::Clear(TRANSPARENT)` into `dst`, full-screen triangle. Caller
/// has already written the params buffer for this frame.
#[allow(clippy::too_many_arguments)]
fn draw_single_pass_treatment(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    dst: &wgpu::TextureView,
    inputs: &TreatmentInputs<'_>,
    pipeline: &wgpu::RenderPipeline,
    bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    params_buf: &wgpu::Buffer,
    label: &'static str,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(inputs.source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: inputs.fit_uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: dst,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..6, 0..1);
}

/// Collage treatment pipeline (P1.3.6). Fixed 2×2 grid of up to four
/// `collage_paths` textures. Empty slots fall back to source (signalled
/// via the `slot_mask` bit in the params uniform). When `inputs.collage`
/// is empty the slot textures are bound to the source view as a
/// harmless placeholder — wgpu requires all bindings populated even if
/// the shader doesn't read them.
struct CollageTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    slot_sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl CollageTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_collage.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/treat_collage.wgsl").into()),
        });

        let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D2,
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        };
        let sampler_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let uniform_entry = |binding: u32, size: u64| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(size),
            },
            count: None,
        };

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_collage bgl"),
            entries: &[
                texture_entry(0),
                sampler_entry(1),
                uniform_entry(2, 16), // fit_uniform: vec4<f32>
                uniform_entry(3, 32), // params: array<vec4<f32>, 2> (PCleanup.8.3b)
                texture_entry(4),
                texture_entry(5),
                texture_entry(6),
                texture_entry(7),
                sampler_entry(8),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_collage pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_collage pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_collage source sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let slot_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_collage slot sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // PCleanup.8.3b: expanded to 32 bytes (array<vec4<f32>, 2>) to
        // accommodate the new `mode` param + mosaic seed offsets.
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_collage params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            slot_sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let mix_amt = inputs.params.get("mix").copied().unwrap_or(0.0);
        let gap = inputs.params.get("gap").copied().unwrap_or(0.02);
        // PCleanup.8.3b: mode 0=grid (default), 1=kaleidoscope, 2=mosaic.
        let col_mode = inputs.params.get("mode").copied().unwrap_or(0.0);

        // Build the slot_mask: bit i set if the slot view is provided.
        let mut mask: u32 = 0;
        for (i, _) in inputs.collage.iter().take(COLLAGE_SLOTS).enumerate() {
            mask |= 1 << i;
        }

        // PCleanup.8.3b — mosaic mode: derive 3 quasi-random f32 offsets from
        // the u64 seed so the shader can compute per-tile UV offsets without
        // any 64-bit WGSL math. Splits the seed into low/high u32 words and
        // applies a simple hash mix (large-prime multiply + xorshift) to
        // produce values well-distributed in [0, 1).
        let seed_lo = inputs.seed as u32;
        let seed_hi = (inputs.seed >> 32) as u32;
        // Mix functions: wrapping multiply by large primes to spread bits.
        let h0 = seed_lo
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(seed_hi.wrapping_mul(0x6c62_272e));
        let h1 = seed_hi
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(seed_lo.wrapping_mul(0x2f4a_d0cf));
        let h2 = h0.wrapping_add(h1).wrapping_mul(0xbf58_476d);
        // Map to [0, 1) via bit-cast to normalised float (u32 / 2^32).
        let r0 = (h0 >> 8) as f32 / 16_777_216.0_f32;
        let r1 = (h1 >> 8) as f32 / 16_777_216.0_f32;
        let r2 = (h2 >> 8) as f32 / 16_777_216.0_f32;

        // Pack 32-byte params uniform: array<vec4<f32>, 2>
        // vec4[0]: [mix, gap, slot_mask_f32, mode]
        // vec4[1]: [seed_r0, seed_r1, seed_r2, _pad]
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&mix_amt.to_le_bytes());
        bytes[4..8].copy_from_slice(&gap.to_le_bytes());
        bytes[8..12].copy_from_slice(&(mask as f32).to_le_bytes());
        bytes[12..16].copy_from_slice(&col_mode.to_le_bytes());
        bytes[16..20].copy_from_slice(&r0.to_le_bytes());
        bytes[20..24].copy_from_slice(&r1.to_le_bytes());
        bytes[24..28].copy_from_slice(&r2.to_le_bytes());
        // bytes[28..32] — padding, left as zero.
        queue.write_buffer(&self.params_buf, 0, &bytes);

        // Slot textures: provided ones use the caller's view; empty
        // slots fall back to the source view (harmless — slot_mask
        // tells the shader to read source for those cells anyway).
        let slot_views: [&wgpu::TextureView; COLLAGE_SLOTS] = [
            inputs.collage.first().copied().unwrap_or(inputs.source),
            inputs.collage.get(1).copied().unwrap_or(inputs.source),
            inputs.collage.get(2).copied().unwrap_or(inputs.source),
            inputs.collage.get(3).copied().unwrap_or(inputs.source),
        ];

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_collage bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(slot_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(slot_views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(slot_views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(slot_views[3]),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&self.slot_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_collage pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        pass.draw(0..6, 0..1);
    }
}

/// Palette-extract / posterize treatment pipeline (P1.3.5).
///
/// PCleanup.8.3a — extended with zone-aware mode. The pipeline now builds
/// its own BGL (no longer delegating to `build_single_pass_treatment`) so
/// it can add:
///   - binding 3: 32-byte params uniform (array<vec4<f32>, 2>) for zone_mode
///     and outside_levels in addition to the original levels/mix/dither fields.
///   - binding 6: 16-byte ZoneTagUniform (zone_tag u32 + 3×u32 padding),
///     following the P3.3.2 slot contract shared by all zone-aware treatments.
///
/// `build_single_pass_treatment` is NOT used here because that helper fixes
/// the params buffer to 16 bytes and has no binding-6 slot.
struct PaletteExtractTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    zone_tag_buf: wgpu::Buffer,
}

impl PaletteExtractTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // PCleanup.8.3a: prepend ZONE_TAG_WGSL at runtime to match
        // build.rs's ZONE_ONLY_CONSUMERS compile-time validation.
        let src = format!(
            "{}\n{}",
            crate::render::sdf::ZONE_TAG_WGSL,
            include_str!("shaders/treat_palette_extract.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_palette_extract.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_palette_extract bgl"),
            entries: &[
                // binding 0: source texture (filterable)
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
                // binding 1: source sampler (filtering)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: fit uniform (16 bytes, vec4<f32>)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // binding 3: params uniform (32 bytes, array<vec4<f32>, 2>)
                // PCleanup.8.3a: expanded from 16 to 32 bytes for zone_mode
                // and outside_levels fields.
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                // binding 6: ZoneTagUniform (16 bytes; P3.3.2 slot contract)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_palette_extract pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_palette_extract pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_palette_extract sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // 32-byte params uniform: array<vec4<f32>, 2>.
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_palette_extract params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 16-byte zone-tag uniform: ZoneTagUniform (u32 zone_tag + 3 × u32 padding).
        let zone_tag_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_palette_extract zone_tag"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            zone_tag_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let levels = inputs.params.get("levels").copied().unwrap_or(4.0);
        let mix_amt = inputs.params.get("mix").copied().unwrap_or(0.0);
        let dither = inputs.params.get("dither").copied().unwrap_or(0.0);
        // PCleanup.8.3a — new zone-aware params. Default zone_mode=0 preserves
        // pre-8.3a behaviour for all existing projects.
        let zone_mode = inputs.params.get("zone_mode").copied().unwrap_or(0.0);
        let outside_levels = inputs.params.get("outside_levels").copied().unwrap_or(4.0);

        // Pack 32-byte params uniform: array<vec4<f32>, 2>
        // vec4[0]: [levels, mix, dither, zone_mode]
        // vec4[1]: [outside_levels, _pad, _pad, _pad]
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&levels.to_le_bytes());
        bytes[4..8].copy_from_slice(&mix_amt.to_le_bytes());
        bytes[8..12].copy_from_slice(&dither.to_le_bytes());
        bytes[12..16].copy_from_slice(&zone_mode.to_le_bytes());
        bytes[16..20].copy_from_slice(&outside_levels.to_le_bytes());
        // bytes[20..32] — padding, left as zero.
        queue.write_buffer(&self.params_buf, 0, &bytes);

        // Write ZoneTagUniform: u32 zone_tag + 3 × u32 padding = 16 bytes.
        let zone_tag = crate::project::schema::zone_role_to_u32(inputs.zone_role);
        let zone_bytes = [
            zone_tag.to_le_bytes(),
            [0u8; 4], // _pad0
            [0u8; 4], // _pad1
            [0u8; 4], // _pad2
        ]
        .concat();
        queue.write_buffer(&self.zone_tag_buf, 0, &zone_bytes);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_palette_extract bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.zone_tag_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_palette_extract pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        pass.draw(0..6, 0..1);
    }
}

/// Texture-overlay treatment pipeline (P1.3.4). Six-binding bind
/// group: source + sampler + fit + params + overlay + overlay-sampler.
/// `inputs.overlay` is the caller-supplied texture view loaded from
/// `Treatment.overlay_path` via the ImageTextureCache.
struct TextureOverlayTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    overlay_sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl TextureOverlayTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_texture_overlay.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/treat_texture_overlay.wgsl").into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_texture_overlay bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_texture_overlay pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_texture_overlay pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_texture_overlay source sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Overlay sampler tiles via Repeat so a small overlay (e.g. a
        // grunge texture) covers the layer through the shader's
        // `fract(uv + offset)` sample.
        let overlay_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_texture_overlay overlay sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_texture_overlay params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            overlay_sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        overlay: &wgpu::TextureView,
    ) {
        let mix_amt = inputs.params.get("mix").copied().unwrap_or(0.0);
        let off_x = inputs.params.get("offset_x").copied().unwrap_or(0.0);
        let off_y = inputs.params.get("offset_y").copied().unwrap_or(0.0);
        let blend = inputs.params.get("blend_mode").copied().unwrap_or(1.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&mix_amt.to_le_bytes());
        bytes[4..8].copy_from_slice(&off_x.to_le_bytes());
        bytes[8..12].copy_from_slice(&off_y.to_le_bytes());
        bytes[12..16].copy_from_slice(&blend.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_texture_overlay bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(overlay),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.overlay_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_texture_overlay pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
        pass.draw(0..6, 0..1);
    }
}

/// Displacement-ripple treatment pipeline (P2.4.1). Single pass, SDF-aware.
/// Displaces the source UV along the SDF normal near the mask boundary,
/// producing a "glass lens at the window edge" refraction effect.
///
/// Bind-group layout (5 entries):
///   0 source texture (filterable)
///   1 filtering sampler (source)
///   2 params uniform    (vec4: amplitude, frequency, decay, _pad)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 SDF texture       (R32Float, NonFiltering)
///
/// The SDF helper (`sdf_helper.wgsl`) is prepended at pipeline build time
/// because the shader's basename starts with `treat_displacement` (see
/// `SDF_CONSUMERS` in build.rs).
struct DisplacementRippleTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl DisplacementRippleTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // Prepend the SDF helper at runtime to match build.rs's compile-time
        // validation (which concatenated it via the SDF_CONSUMERS rule).
        let src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_displacement_ripple.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_displacement_ripple.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_displacement_ripple bgl"),
            entries: &[
                // binding 0: source texture (filterable RGBA)
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
                // binding 1: filtering sampler (source)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (16 bytes)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // binding 3: fit uniform (16 bytes)
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
                // binding 4: SDF texture — R32Float, NonFiltering.
                // sdf_helper uses textureLoad so no sampler slot is needed.
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_displacement_ripple pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_displacement_ripple pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_displacement_ripple source sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_displacement_ripple params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        // Resolve params: fall back to descriptor defaults when keys are absent.
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);
        let frequency = inputs.params.get("frequency").copied().unwrap_or(8.0);
        let decay = inputs.params.get("decay").copied().unwrap_or(0.5);

        // Pack into 16-byte uniform: [amplitude, frequency, decay, _pad].
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&amplitude.to_le_bytes());
        bytes[4..8].copy_from_slice(&frequency.to_le_bytes());
        bytes[8..12].copy_from_slice(&decay.to_le_bytes());
        // bytes[12..16] reserved (zero).
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_displacement_ripple bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_displacement_ripple pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// PCleanup.2.1 — Ripple-lens treatment pipeline. Concentric-ring UV
/// displacement keyed to SDF distance — the SourceModifier sibling of
/// the generative `mask_edge_ripple_wash` FX preset.
///
/// Structurally identical to `DisplacementRippleTreatmentPipeline`
/// (same 5-binding layout: source / sampler / params / fit / sdf);
/// only the shader differs. Could be DRY'd with the displacement_ripple
/// constructor if the W2 sibling treatments grow numerous; for now
/// each preset gets its own struct in the existing four-file pattern.
struct RippleLensTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl RippleLensTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_ripple_lens.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_ripple_lens.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        // Bind-group layout matches treat_displacement_ripple bit-for-bit
        // (same 5 bindings). See DisplacementRippleTreatmentPipeline for
        // the full layout commentary.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_ripple_lens bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_ripple_lens pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_ripple_lens pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_ripple_lens sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_ripple_lens params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);
        let wavelength = inputs.params.get("wavelength").copied().unwrap_or(0.08);
        let speed = inputs.params.get("speed").copied().unwrap_or(0.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&amplitude.to_le_bytes());
        bytes[4..8].copy_from_slice(&wavelength.to_le_bytes());
        bytes[8..12].copy_from_slice(&speed.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_ripple_lens bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_ripple_lens pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// PCleanup.2.2 — `edge_lens` treatment pipeline. Single fragment pass,
/// SDF-aware. Uses the SDF normal direction to compute an angular
/// position around the mask boundary; N traveling sine crests displace
/// the source UV radially. Identity at `amplitude = 0.0`.
///
/// Bind-group layout matches `ripple_lens` / `displacement_ripple`:
///   0 source texture (filterable)
///   1 filtering sampler (source)
///   2 params uniform    (vec4: amplitude, n_waves, speed, clock_secs)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 SDF texture       (R32Float, NonFiltering)
///
/// The `clock_secs` field in slot `w` of the params uniform is written
/// each frame by `render` from `TreatmentInputs::clock_secs` — there is
/// no operator-facing slider for it (advancing time animates the crests
/// around the boundary).
struct EdgeLensTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl EdgeLensTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_edge_lens.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_edge_lens.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_edge_lens bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_edge_lens pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_edge_lens pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_edge_lens sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_edge_lens params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);
        let n_waves = inputs.params.get("n_waves").copied().unwrap_or(4.0);
        let speed = inputs.params.get("speed").copied().unwrap_or(1.0);

        // clock_secs is packed into the params uniform's `w` slot (no
        // separate binding) so the shader can compute crest travel
        // without an extra uniform buffer.
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&amplitude.to_le_bytes());
        bytes[4..8].copy_from_slice(&n_waves.to_le_bytes());
        bytes[8..12].copy_from_slice(&speed.to_le_bytes());
        bytes[12..16].copy_from_slice(&inputs.clock_secs.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_edge_lens bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_edge_lens pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// PCleanup.2.7 — `field_advect_source` treatment pipeline. Single fragment
/// pass, SDF-aware. Advects the source image along the SDF gradient field:
/// samples t_source at `uv - gradient(uv) * flow_speed * clock_secs`.
/// Identity at `flow_speed = 0.0` (offset collapses to vec2(0)).
///
/// Bind-group layout matches `edge_lens` / `displacement_ripple`:
///   0 source texture (filterable)
///   1 filtering sampler (source)
///   2 params uniform    (vec4: flow_speed, _pad, _pad, clock_secs)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 SDF texture       (R32Float, NonFiltering)
///
/// The `clock_secs` field in slot `w` of the params uniform is written
/// each frame by `render` from `TreatmentInputs::clock_secs` — there is
/// no operator-facing slider for it.
struct FieldAdvectTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl FieldAdvectTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_field_advect.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_field_advect.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_field_advect bgl"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_field_advect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_field_advect pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_field_advect sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_field_advect params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let flow_speed = inputs.params.get("flow_speed").copied().unwrap_or(0.0);

        // clock_secs is packed into the params uniform's `w` slot (no
        // separate binding) so the shader can compute the advection offset
        // without an extra uniform buffer.
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&flow_speed.to_le_bytes());
        // bytes[4..8] and [8..12] are _pad, left as zero.
        bytes[12..16].copy_from_slice(&inputs.clock_secs.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_field_advect bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_field_advect pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// PCleanup.1.2 — `fluid_warp` treatment pipeline. Two-pass per frame:
///   1. Compute pre-pass: bounded-fluid advection via an owned `FxFluidPipeline`
///      (SDF boundary zeroes velocity outside the mask, so warping is naturally
///      constrained to the masked region).
///   2. Fragment pass: samples `t_source` at `uv - velocity * amplitude`.
///
/// The `FxFluidPipeline` instance here is independent of the one owned by
/// `FxPipelines::bounded_fluid` in `fx_presets.rs` — they run separate
/// simulations and do not share state.
///
/// Note: multiple layers applying `fluid_warp` in the same frame will each
/// dispatch their own advect step (one per layer). This is intentional for
/// this PR; per-layer sim isolation is a separate design conversation.
///
/// Bind-group layout for the fragment pass (5 entries):
///   0 source texture (filterable)
///   1 filtering sampler (source + velocity — RGBA16Float is filterable)
///   2 params uniform    (vec4: amplitude, _pad, _pad, clock_secs)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 velocity texture  (RGBA16Float, filterable — written by compute pre-pass)
///
/// No SDF_CONSUMERS entry in build.rs: the fragment shader does not call
/// any sdf_helper function; the compute side handles boundary via dispatch.
struct FluidWarpTreatmentPipeline {
    /// Owned bounded-fluid simulation; independent from FxPipelines.
    bounded_fluid: crate::render::fx_fluid::FxFluidPipeline,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl FluidWarpTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bounded_fluid =
            crate::render::fx_fluid::FxFluidPipeline::new_bounded_fluid(device, target_format);

        // Fragment shader — does NOT need the SDF helper prepended.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_fluid_warp.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/treat_fluid_warp.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_fluid_warp bgl"),
            entries: &[
                // binding 0: source texture (filterable)
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
                // binding 1: filtering sampler (shared for source + velocity)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (amplitude, _pad, _pad, clock_secs)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // binding 3: fit uniform (mode, aspect, focal_x, focal_y)
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
                // binding 4: velocity texture (RGBA16Float, filterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_fluid_warp pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_fluid_warp pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_fluid_warp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_fluid_warp params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            bounded_fluid,
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);

        // --- Compute pre-pass: advance the bounded-fluid sim one step. ---
        // inject_intensity=0.4 seeds a steady swirl at the mask centre so
        // there is visible motion when amplitude > 0 (matches the value used
        // by FxPipelines::bounded_fluid in fx_presets.rs).
        // dissipation=0.95 — mild energy loss per step (same as bounded_fluid
        // preset default) so the field stays visually active without diverging.
        self.bounded_fluid.dispatch_advect(
            device,
            queue,
            encoder,
            Some(sdf),
            inputs.clock_secs,
            0.95,
            0.4,
        );

        // After dispatch_advect, `current_velocity_view()` returns the
        // just-written texture (parity-aware).
        let vel_view = self.bounded_fluid.current_velocity_view();

        // Pack params uniform: x=amplitude, w=clock_secs (per-frame, not
        // operator-facing), y/z padding.
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&amplitude.to_le_bytes());
        bytes[12..16].copy_from_slice(&inputs.clock_secs.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_fluid_warp bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(vel_view),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_fluid_warp pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// PCleanup.2.3 — `fluid_warp_full` treatment pipeline. Two-pass per frame:
///   1. Compute pre-pass: fluid_identity advection via an owned `FxFluidPipeline`
///      (no SDF boundary — velocity covers the full layer rect).
///   2. Fragment pass: samples `t_source` at `uv - velocity * amplitude`.
///
/// Unbounded sibling of `FluidWarpTreatmentPipeline` (PCleanup.1.2).
/// Swaps `bounded_fluid` → `fluid_identity` (constructed via
/// `FxFluidPipeline::new_fluid_identity`) and drops the SDF parameter from
/// `render()` — dispatch requires no SDF texture.
///
/// The `FxFluidPipeline` instance here is independent of the one owned by
/// `FxPipelines::fluid_identity` in `fx_presets.rs`.
///
/// Bind-group layout for the fragment pass (5 entries):
///   0 source texture (filterable)
///   1 filtering sampler (source + velocity — RGBA16Float is filterable)
///   2 params uniform    (vec4: amplitude, _pad, _pad, clock_secs)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 velocity texture  (RGBA16Float, filterable — written by compute pre-pass)
///
/// No SDF_CONSUMERS entry in build.rs: the fragment shader does not call
/// any sdf_helper function; the compute side is SDF-free.
struct FluidWarpFullTreatmentPipeline {
    /// Owned fluid_identity simulation; independent from FxPipelines.
    fluid_identity: crate::render::fx_fluid::FxFluidPipeline,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl FluidWarpFullTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let fluid_identity =
            crate::render::fx_fluid::FxFluidPipeline::new_fluid_identity(device, target_format);

        // Fragment shader — does NOT need the SDF helper prepended.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_fluid_warp_full.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/treat_fluid_warp_full.wgsl").into(),
            ),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_fluid_warp_full bgl"),
            entries: &[
                // binding 0: source texture (filterable)
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
                // binding 1: filtering sampler (shared for source + velocity)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (amplitude, _pad, _pad, clock_secs)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // binding 3: fit uniform (mode, aspect, focal_x, focal_y)
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
                // binding 4: velocity texture (RGBA16Float, filterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_fluid_warp_full pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_fluid_warp_full pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_fluid_warp_full sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_fluid_warp_full params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            fluid_identity,
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);

        // --- Compute pre-pass: advance the fluid_identity sim one step. ---
        // sdf_view=None — fluid_identity has no mask dependency; velocity
        // evolves freely across the full 256×256 grid.
        // inject_intensity=0.5 matches the value used by FxPipelines::fluid_identity
        // in fx_presets.rs (seeds a steady swirl so motion is visible at amplitude > 0).
        // dissipation=0.95 — mild energy loss per step; keeps the field visually
        // active and smooth without diverging.
        self.fluid_identity.dispatch_advect(
            device,
            queue,
            encoder,
            None, // sdf_view: fluid_identity does not use mask boundary
            inputs.clock_secs,
            0.95,
            0.5,
        );

        // After dispatch_advect, `current_velocity_view()` returns the
        // just-written texture (parity-aware).
        let vel_view = self.fluid_identity.current_velocity_view();

        // Pack params uniform: x=amplitude, w=clock_secs (per-frame, not
        // operator-facing), y/z padding.
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&amplitude.to_le_bytes());
        bytes[12..16].copy_from_slice(&inputs.clock_secs.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_fluid_warp_full bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(vel_view),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_fluid_warp_full pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Refraction treatment pipeline (P2.4.2). Single pass, SDF-aware.
/// Bends the source UV along the SDF normal near the mask boundary using a
/// Snell-like offset, producing a glass-lens refraction effect at the edge.
///
/// Bind-group layout (5 entries) — identical to DisplacementRippleTreatmentPipeline:
///   0 source texture (filterable)
///   1 filtering sampler (source)
///   2 params uniform    (vec4: ior, edge_width, _pad, _pad)
///   3 fit uniform       (vec4: mode, aspect, focal_x, focal_y)
///   4 SDF texture       (R32Float, NonFiltering)
///
/// The SDF helper (`sdf_helper.wgsl`) is prepended at pipeline build time
/// because the shader's basename starts with `treat_refraction` (see
/// `SDF_CONSUMERS` in build.rs).
struct RefractionTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl RefractionTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // Prepend the SDF helper at runtime to match build.rs's compile-time
        // validation (which concatenated it via the SDF_CONSUMERS rule).
        let src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_refraction.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_refraction.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_refraction bgl"),
            entries: &[
                // binding 0: source texture (filterable RGBA)
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
                // binding 1: filtering sampler (source)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (16 bytes)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // binding 3: fit uniform (16 bytes)
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
                // binding 4: SDF texture — R32Float, NonFiltering.
                // sdf_helper uses textureLoad so no sampler slot is needed.
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_refraction pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_refraction pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_refraction source sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_refraction params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        // Resolve params: fall back to descriptor defaults when keys are absent.
        let ior = inputs.params.get("ior").copied().unwrap_or(1.0);
        let edge_width = inputs.params.get("edge_width").copied().unwrap_or(0.1);

        // Pack into 16-byte uniform: [ior, edge_width, _pad, _pad].
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&ior.to_le_bytes());
        bytes[4..8].copy_from_slice(&edge_width.to_le_bytes());
        // bytes[8..16] reserved (zero).
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_refraction bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_refraction pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

// ---------------------------------------------------------------------------
// PCleanup.2.9 — `zone_brighten` treatment pipeline
// ---------------------------------------------------------------------------
//
// Single-pass fragment shader. Reads `source` and the layer SDF, samples the
// zone-tag uniform (slot 6), and multiplicatively boosts source luminance
// inside the ZONE_WINDOW-tagged polygon area with an exponential edge falloff
// matching `fx_zone_light_spill`. Outside ZONE_WINDOW the shader passes
// source through unchanged — no crash, no visible effect.
//
// Pipeline fields:
//   `params_buf` — 32-byte uniform (intensity, falloff, spill_radius, speed,
//                  clock_secs, pad×3). Written per render call.
//   `zone_tag_buf` — 16-byte ZoneTagUniform (zone_tag u32 + 3×u32 pad).
//                   Written from `inputs.zone_role` per render call.
//   Slot 6 is in the bind-group layout — follows the zone-aware slot table
//   documented in P3.3.2 and mirrored in FxPresetPipeline::new_zone_aware.
//
// Blend mode: ALPHA_BLENDING (same as field_advect / ripple_lens). Source
// pixels whose alpha < 1 will composite correctly; the multiplier is applied
// before blending.

struct ZoneBrightenTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    zone_tag_buf: wgpu::Buffer,
}

impl ZoneBrightenTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // PCleanup.2.9: prepend SDF helper + zone-tag helper at module-create
        // time (same pattern as FxPresetPipeline::new_zone_light_spill).
        // build.rs also prepends these for standalone naga validation.
        let src = format!(
            "{}\n{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            crate::render::sdf::ZONE_TAG_WGSL,
            include_str!("shaders/treat_zone_brighten.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_zone_brighten.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_zone_brighten bgl"),
            entries: &[
                // binding 0: source texture (filterable — RGBA8 / Bgra8)
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
                // binding 1: source sampler (filtering)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (32 bytes — ZoneBrightenParams struct)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                // binding 3: SDF texture (R32Float, non-filterable; textureLoad only)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // binding 6: ZoneTagUniform (16 bytes; P3.3.2 slot contract)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_zone_brighten pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_zone_brighten pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_zone_brighten sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // 32-byte params uniform: ZoneBrightenParams struct (8 × f32).
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_zone_brighten params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 16-byte zone-tag uniform: ZoneTagUniform (u32 zone_tag + 3 × u32 pad).
        let zone_tag_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_zone_brighten zone_tag"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            zone_tag_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let intensity = inputs.params.get("intensity").copied().unwrap_or(0.0);
        let falloff = inputs.params.get("falloff").copied().unwrap_or(8.0);
        let spill_radius = inputs.params.get("spill_radius").copied().unwrap_or(0.3);
        let speed = inputs.params.get("speed").copied().unwrap_or(0.0);
        let clock_secs = inputs.clock_secs;

        // Write ZoneBrightenParams: 8 × f32 = 32 bytes, little-endian.
        // Layout: [intensity, falloff, spill_radius, speed, clock_secs, pad, pad, pad]
        let mut params_bytes = [0u8; 32];
        let floats = [
            intensity,
            falloff,
            spill_radius,
            speed,
            clock_secs,
            0.0f32,
            0.0,
            0.0,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // Write ZoneTagUniform: u32 zone_tag + 3 × u32 padding = 16 bytes.
        let zone_tag = crate::project::schema::zone_role_to_u32(inputs.zone_role);
        let zone_bytes = [
            zone_tag.to_le_bytes(),
            [0u8; 4], // _pad0
            [0u8; 4], // _pad1
            [0u8; 4], // _pad2
        ]
        .concat();
        queue.write_buffer(&self.zone_tag_buf, 0, &zone_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_zone_brighten bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.zone_tag_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_zone_brighten pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

// PCleanup.2.10 — `zone_lens` treatment pipeline
// ---------------------------------------------------------------------------
//
// Single-pass fragment shader. Reads `source` and the layer SDF, samples the
// zone-tag uniform (slot 6), and displaces source UV coordinates in a thin
// exponential band around the ZONE_WINDOW polygon edge, creating a lens /
// refraction effect. Mirrors the spatial band shape of `fx_zone_edge_ripple`;
// outside ZONE_WINDOW the shader passes source through unchanged.
//
// Pipeline fields:
//   `params_buf`  — 32-byte uniform (amplitude, speed, band_width, frequency,
//                   clock_secs, pad×3). Written per render call.
//   `zone_tag_buf` — 16-byte ZoneTagUniform (zone_tag u32 + 3×u32 pad).
//                   Written from `inputs.zone_role` per render call.
//   Slot 6 follows the zone-aware slot table from P3.3.2, matching
//   ZoneBrightenTreatmentPipeline exactly.
//
// Blend mode: ALPHA_BLENDING — same as zone_brighten / ripple_lens.

struct ZoneLensTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    zone_tag_buf: wgpu::Buffer,
}

impl ZoneLensTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // PCleanup.2.10: prepend SDF helper + zone-tag helper at module-create
        // time (same pattern as ZoneBrightenTreatmentPipeline::new).
        // build.rs also prepends these for standalone naga validation.
        let src = format!(
            "{}\n{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            crate::render::sdf::ZONE_TAG_WGSL,
            include_str!("shaders/treat_zone_lens.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_zone_lens.wgsl"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_zone_lens bgl"),
            entries: &[
                // binding 0: source texture (filterable — RGBA8 / Bgra8)
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
                // binding 1: source sampler (filtering)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: params uniform (32 bytes — ZoneLensParams struct)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                // binding 3: SDF texture (R32Float, non-filterable; textureLoad only)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // binding 6: ZoneTagUniform (16 bytes; P3.3.2 slot contract)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_zone_lens pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_zone_lens pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_zone_lens sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // 32-byte params uniform: ZoneLensParams struct (8 × f32).
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_zone_lens params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 16-byte zone-tag uniform: ZoneTagUniform (u32 zone_tag + 3 × u32 padding).
        let zone_tag_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_zone_lens zone_tag"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            zone_tag_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
    ) {
        let amplitude = inputs.params.get("amplitude").copied().unwrap_or(0.0);
        let speed = inputs.params.get("speed").copied().unwrap_or(1.0);
        let band_width = inputs.params.get("band_width").copied().unwrap_or(0.05);
        let frequency = inputs.params.get("frequency").copied().unwrap_or(10.0);
        let clock_secs = inputs.clock_secs;

        // Write ZoneLensParams: 8 × f32 = 32 bytes, little-endian.
        // Layout: [amplitude, speed, band_width, frequency, clock_secs, pad, pad, pad]
        let mut params_bytes = [0u8; 32];
        let floats = [
            amplitude, speed, band_width, frequency, clock_secs, 0.0f32, 0.0, 0.0,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // Write ZoneTagUniform: u32 zone_tag + 3 × u32 padding = 16 bytes.
        let zone_tag = crate::project::schema::zone_role_to_u32(inputs.zone_role);
        let zone_bytes = [
            zone_tag.to_le_bytes(),
            [0u8; 4], // _pad0
            [0u8; 4], // _pad1
            [0u8; 4], // _pad2
        ]
        .concat();
        queue.write_buffer(&self.zone_tag_buf, 0, &zone_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_zone_lens bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.zone_tag_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_zone_lens pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

// ---------------------------------------------------------------------------
// B.1 — Preset capability metadata
// ---------------------------------------------------------------------------

/// T1.20 — Capability flags and headline parameter for a treatment preset.
///
/// Used by the Look-chain UI to show status dots, autofix chips, and
/// headline-param-on-row without needing a `match` at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetCapability {
    /// True when the preset reads the layer's SDF texture and produces a
    /// passthrough when `warp.mask_polygon` is empty.
    pub requires_sdf: bool,
    /// True when the preset reads `warp.zone_role` and is a no-op when it
    /// is `None`.
    pub requires_zone: bool,
    /// True when the preset runs a particle compute pass (spotlights family
    /// and edge-spawning siblings).
    pub is_particle: bool,
    /// The most operator-relevant tunable parameter key, shown on the chain
    /// row as a compact slider. `None` for identity and presets with no
    /// obvious single headline (e.g. pure multi-param presets where no one
    /// param dominates).
    pub headline_param: Option<&'static str>,
}

/// T1.20 — Returns the [`PresetCapability`] for the given `preset_id`.
///
/// Returns all-defaults (no SDF/zone/particle, no headline) for unknown
/// preset ids so callers degrade gracefully on hand-edited or future presets.
#[allow(dead_code)] // consumed by look_chain UI (Phase 1 C)
pub fn capability(preset_id: &str) -> PresetCapability {
    match preset_id {
        // ---- SDF-keyed source modifiers ----
        RIPPLE_LENS_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        DISPLACEMENT_RIPPLE_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        EDGE_LENS_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        FIELD_ADVECT_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("flow_speed"),
        },
        REFRACTION_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("ior"),
        },
        BLUR_MASK_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("max_radius_px"),
        },
        FLUID_WARP_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        // ---- SDF + zone ----
        ZONE_BRIGHTEN_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: true,
            is_particle: false,
            headline_param: Some("intensity"),
        },
        ZONE_LENS_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: true,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        // ---- SDF + particle ----
        SPOTLIGHTS_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: true,
            headline_param: Some("particle_count"),
        },
        DRIFT_PINHOLES_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: true,
            headline_param: Some("particle_count"),
        },
        DRIFT_BRUSHSTROKES_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: true,
            headline_param: Some("particle_count"),
        },
        EDGE_SPARKS_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: true,
            headline_param: Some("particle_count"),
        },
        COLLISION_RIPPLES_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: true,
            headline_param: Some("particle_count"),
        },
        PORTAL_WARP_PRESET_ID => PresetCapability {
            requires_sdf: true,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("particle_count"),
        },
        // ---- Non-SDF: utility / colour-grading ----
        FLUID_WARP_FULL_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("amplitude"),
        },
        TONE_MAP_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("exposure"),
        },
        LUMINANCE_REVEAL_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("threshold"),
        },
        TEXTURE_OVERLAY_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("mix"),
        },
        PALETTE_EXTRACT_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("levels"),
        },
        COLLAGE_PRESET_ID => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: Some("mix"),
        },
        // identity and any unknown preset
        _ => PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: None,
        },
    }
}

// ---------------------------------------------------------------------------
// B.2 — No-op detection
// ---------------------------------------------------------------------------

/// T1.21 — Returns a human-readable reason string when applying `preset_id`
/// with the given `params` to `layer` will produce a no-op (passthrough)
/// output.  Returns `None` when the combination looks useful.
///
/// This function only covers the cases detectable from `(preset_id, params,
/// LayerConfig)`.  The `texture_overlay` "Overlay file missing" case requires
/// `Effect::Treatment.overlay_path` which is not visible here; that check
/// lives in `effect_is_no_op` in `src/effects/mod.rs`.
///
/// # Cases covered
/// - `requires_sdf(preset) && layer.warp.mask_polygon.is_empty()` → `"Needs a mask polygon"`
/// - `requires_zone(preset) && layer.warp.zone_role.is_none()` → `"Needs a zone role"`
/// - `ripple_lens` with `|amplitude| < 1e-4` → `"Amplitude at 0"`
/// - `tone_map` at identity params → `"All params at identity"`
#[allow(dead_code)] // wired by look_chain UI (Phase 1 C)
pub fn treatment_is_no_op(
    preset_id: &str,
    params: &HashMap<String, f32>,
    layer: &crate::project::schema::LayerConfig,
) -> Option<&'static str> {
    let cap = capability(preset_id);

    if cap.requires_sdf && layer.warp.mask_polygon.is_empty() {
        return Some("Needs a mask polygon");
    }

    if cap.requires_zone && layer.warp.zone_role.is_none() {
        return Some("Needs a zone role");
    }

    if preset_id == RIPPLE_LENS_PRESET_ID {
        let amplitude = params.get("amplitude").copied().unwrap_or(0.0);
        if amplitude.abs() < 1e-4 {
            return Some("Amplitude at 0");
        }
    }

    if preset_id == TONE_MAP_PRESET_ID {
        let exposure = params.get("exposure").copied().unwrap_or(0.0);
        let contrast = params.get("contrast").copied().unwrap_or(1.0);
        let shoulder = params.get("shoulder").copied().unwrap_or(0.0);
        if exposure.abs() < 1e-6 && (contrast - 1.0).abs() < 1e-6 && shoulder.abs() < 1e-6 {
            return Some("All params at identity");
        }
    }

    None
}

// ---------------------------------------------------------------------------
// 004-T1.23 — Intent group per preset
// ---------------------------------------------------------------------------

/// 004-T1.23 — Maps a treatment `preset_id` to its [`crate::effects::IntentGroup`].
///
/// Returns `IntentGroup::Compose` for unknown preset ids so the picker
/// degrades gracefully when a project contains a hand-edited or future preset.
///
/// Mapping rationale (from spec 004-treatment-overhaul.md §B.3):
/// - **Warp**: spatially displaces the source UV coordinates.
/// - **Color**: tone/luminance grading, palette shaping.
/// - **Texture**: composites external textures or uses the SDF as a blur mask.
/// - **Compose**: utility / passthrough.
/// - **Animate**: particle-driven motion effects that modulate source pixels.
#[allow(dead_code)] // consumed by effects::intent_group (Phase 1 T1.23)
pub fn intent_group_for_preset(preset_id: &str) -> crate::effects::IntentGroup {
    use crate::effects::IntentGroup;
    match preset_id {
        // Warp — displaces source UVs
        FLUID_WARP_PRESET_ID
        | FLUID_WARP_FULL_PRESET_ID
        | RIPPLE_LENS_PRESET_ID
        | EDGE_LENS_PRESET_ID
        | FIELD_ADVECT_PRESET_ID
        | DISPLACEMENT_RIPPLE_PRESET_ID
        | REFRACTION_PRESET_ID
        | ZONE_LENS_PRESET_ID
        | PORTAL_WARP_PRESET_ID => IntentGroup::Warp,
        // Color — tone / luminance / palette grading
        TONE_MAP_PRESET_ID
        | LUMINANCE_REVEAL_PRESET_ID
        | PALETTE_EXTRACT_PRESET_ID
        | ZONE_BRIGHTEN_PRESET_ID => IntentGroup::Color,
        // Texture — external texture compositing or SDF-gated blur
        TEXTURE_OVERLAY_PRESET_ID | BLUR_MASK_PRESET_ID => IntentGroup::Texture,
        // Compose — passthrough / collage utilities
        IDENTITY_PRESET_ID | COLLAGE_PRESET_ID => IntentGroup::Compose,
        // Animate — particle-driven source modulation
        SPOTLIGHTS_PRESET_ID
        | DRIFT_PINHOLES_PRESET_ID
        | DRIFT_BRUSHSTROKES_PRESET_ID
        | EDGE_SPARKS_PRESET_ID
        | COLLISION_RIPPLES_PRESET_ID => IntentGroup::Animate,
        // Unknown / future presets fall back to Compose (visible but neutral).
        _ => IntentGroup::Compose,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acceptance: the identity preset is registered.
    #[test]
    fn identity_preset_is_registered() {
        assert!(is_registered(IDENTITY_PRESET_ID));
    }

    /// Acceptance: the tone_map preset is registered (P1.3.1).
    #[test]
    fn tone_map_preset_is_registered() {
        assert!(is_registered(TONE_MAP_PRESET_ID));
    }

    /// Acceptance: tone_map's three documented params have descriptors,
    /// and all defaults round-trip through the identity case (exposure=0,
    /// contrast=1, shoulder=0 ⇒ shader passthrough).
    #[test]
    fn tone_map_descriptor_defaults_are_identity() {
        let descriptors = param_descriptors(TONE_MAP_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "tone_map exposes exactly 3 params");

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["exposure"].default, 0.0, "exposure identity = 0");
        assert_eq!(by_key["contrast"].default, 1.0, "contrast identity = 1");
        assert_eq!(by_key["shoulder"].default, 0.0, "shoulder identity = 0");

        // Min/max sanity: ranges are non-degenerate and bracket the default.
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the luminance_reveal preset is registered (P1.3.3).
    #[test]
    fn luminance_reveal_preset_is_registered() {
        assert!(is_registered(LUMINANCE_REVEAL_PRESET_ID));
    }

    /// Acceptance: luminance_reveal exposes threshold + softness + invert
    /// with sane defaults (50 % threshold, gentle softness, non-inverted).
    #[test]
    fn luminance_reveal_descriptor_defaults_are_sane() {
        let descriptors = param_descriptors(LUMINANCE_REVEAL_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "luminance_reveal exposes 3 params");

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["threshold"].default, 0.5, "threshold default = 0.5");
        assert_eq!(by_key["softness"].default, 0.1, "softness default = 0.1");
        assert_eq!(by_key["invert"].default, 0.0, "invert default = 0.0 (off)");

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the blur_mask preset is registered (P1.3.2).
    #[test]
    fn blur_mask_preset_is_registered() {
        assert!(is_registered(BLUR_MASK_PRESET_ID));
    }

    /// Acceptance: blur_mask exposes max_radius_px + edge_band + falloff
    /// plus PCleanup.8.3c additions (radius_mode + distance_falloff).
    /// The default `max_radius_px = 0` is the key identity property —
    /// operator sees no change until they reach for the radius slider.
    /// The default `radius_mode = 0` preserves pre-8.3c behaviour exactly.
    #[test]
    fn blur_mask_defaults_are_no_op() {
        let descriptors = param_descriptors(BLUR_MASK_PRESET_ID);
        assert_eq!(descriptors.len(), 5);

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(
            by_key["max_radius_px"].default, 0.0,
            "max_radius identity = 0 (no blur)"
        );
        assert!(by_key["edge_band"].default > 0.0);
        assert!(by_key["falloff"].default >= 0.0 && by_key["falloff"].default <= 1.0);
        // PCleanup.8.3c: radius_mode default=0 (edge-band, existing behaviour).
        assert_eq!(
            by_key["radius_mode"].default, 0.0,
            "radius_mode default = 0 preserves pre-8.3c behaviour"
        );
        assert!(by_key["distance_falloff"].default > 0.0);

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the texture_overlay preset is registered (P1.3.4).
    #[test]
    fn texture_overlay_preset_is_registered() {
        assert!(is_registered(TEXTURE_OVERLAY_PRESET_ID));
    }

    /// Acceptance: texture_overlay defaults give an identity (mix=0).
    #[test]
    fn texture_overlay_defaults_are_identity() {
        let descriptors = param_descriptors(TEXTURE_OVERLAY_PRESET_ID);
        assert_eq!(descriptors.len(), 4);
        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["mix"].default, 0.0, "mix=0 → identity");
    }

    /// Acceptance: unknown preset ids are not registered.
    #[test]
    fn unknown_preset_is_not_registered() {
        assert!(!is_registered(""));
        assert!(!is_registered("definitely-not-a-real-preset"));
        assert!(!is_registered("foo_unknown")); // not a real preset
    }

    /// Acceptance: every registered preset has an entry in
    /// `param_descriptors` (even if empty). Guards against W3 adding a
    /// registry entry but forgetting to wire descriptors.
    #[test]
    fn every_registered_preset_has_descriptor_entry() {
        for (id, _label) in registry() {
            // No panic = ok; identity returns &[], which is the documented
            // behaviour for presets with no tunable params.
            let _ = param_descriptors(id);
        }
    }

    /// Acceptance: the registry has no duplicate preset_ids. Catches a W3
    /// copy-paste mistake before it ships.
    #[test]
    fn registry_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (id, _) in registry() {
            assert!(seen.insert(*id), "duplicate preset_id in registry: {id}");
        }
    }

    /// Acceptance: the displacement_ripple preset is registered (P2.4.1).
    #[test]
    fn displacement_ripple_preset_is_registered() {
        assert!(is_registered(DISPLACEMENT_RIPPLE_PRESET_ID));
    }

    /// Acceptance: displacement_ripple exposes amplitude + frequency + decay
    /// with valid min/max/default ranges and 3 total descriptors.
    #[test]
    fn displacement_ripple_descriptors_present() {
        let descriptors = param_descriptors(DISPLACEMENT_RIPPLE_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            3,
            "displacement_ripple exposes exactly 3 params"
        );

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the amplitude descriptor defaults to 0.0, satisfying the
    /// identity-default rule (amplitude=0 → disp=vec2(0) → passthrough).
    #[test]
    fn displacement_ripple_amplitude_default_is_zero() {
        let descriptors = param_descriptors(DISPLACEMENT_RIPPLE_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(
            amplitude_desc.default, 0.0,
            "amplitude identity default = 0.0"
        );
    }

    /// Acceptance: the refraction preset is registered (P2.4.2).
    #[test]
    fn refraction_preset_is_registered() {
        assert!(is_registered(REFRACTION_PRESET_ID));
    }

    // ----- PCleanup.2.1 — ripple_lens treatment ----------------------

    /// PCleanup.2.1 — `ripple_lens` is registered in the treatment list
    /// and visible to the picker via `is_registered`.
    #[test]
    fn ripple_lens_is_registered() {
        assert!(
            is_registered(RIPPLE_LENS_PRESET_ID),
            "ripple_lens must appear in treatments::registry()"
        );
    }

    /// PCleanup.2.1 — `ripple_lens` exposes amplitude + wavelength +
    /// speed with valid descriptor ranges; total 3 entries.
    #[test]
    fn ripple_lens_descriptors_present() {
        let descriptors = param_descriptors(RIPPLE_LENS_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "ripple_lens exposes exactly 3 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// PCleanup.2.1 — `ripple_lens` `amplitude` default = 0.0 so a
    /// freshly-added Treatment is bit-identical passthrough (matches
    /// the "inert on add" rule the other SourceModifier treatments follow).
    #[test]
    fn ripple_lens_amplitude_default_is_zero() {
        let descriptors = param_descriptors(RIPPLE_LENS_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(amplitude_desc.default, 0.0);
    }

    /// PCleanup.2.2 — `edge_lens` registered.
    #[test]
    fn edge_lens_is_registered() {
        assert!(
            is_registered(EDGE_LENS_PRESET_ID),
            "edge_lens must appear in treatments::registry()"
        );
    }

    /// PCleanup.2.2 — `edge_lens` exposes amplitude + n_waves + speed.
    #[test]
    fn edge_lens_descriptors_present() {
        let descriptors = param_descriptors(EDGE_LENS_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "edge_lens exposes exactly 3 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// PCleanup.2.2 — `edge_lens` `amplitude` default = 0.0 → inert on add.
    #[test]
    fn edge_lens_amplitude_default_is_zero() {
        let descriptors = param_descriptors(EDGE_LENS_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(amplitude_desc.default, 0.0);
    }

    /// Acceptance: refraction exposes ior + edge_width with valid
    /// min/max/default ranges and exactly 2 total descriptors.
    #[test]
    fn refraction_descriptors_present() {
        let descriptors = param_descriptors(REFRACTION_PRESET_ID);
        assert_eq!(descriptors.len(), 2, "refraction exposes exactly 2 params");

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the ior descriptor defaults to 1.0, satisfying the
    /// identity-default rule (ior=1.0 → bend=vec2(0) → passthrough).
    #[test]
    fn refraction_ior_default_is_one() {
        let descriptors = param_descriptors(REFRACTION_PRESET_ID);
        let ior_desc = descriptors
            .iter()
            .find(|d| d.key == "ior")
            .expect("ior descriptor must be present");
        assert_eq!(ior_desc.default, 1.0, "ior identity default = 1.0");
    }

    // ----- PCleanup.2.7 — field_advect_source treatment ------------------

    /// PCleanup.2.7 — `field_advect_source` is registered in the treatment
    /// list so the operator can apply it from the picker.
    #[test]
    fn field_advect_is_registered() {
        assert!(
            is_registered(FIELD_ADVECT_PRESET_ID),
            "field_advect_source must appear in treatments::registry()"
        );
    }

    /// PCleanup.2.7 — `field_advect_source` exposes exactly one operator
    /// param (`flow_speed`) with a valid min/max/default range.
    #[test]
    fn field_advect_descriptors_present() {
        let descriptors = param_descriptors(FIELD_ADVECT_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            1,
            "field_advect_source exposes exactly 1 param"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// PCleanup.2.7 — `field_advect_source` `flow_speed` default = 0.0
    /// satisfies the identity-default rule.
    #[test]
    fn field_advect_flow_speed_default_is_zero() {
        let descriptors = param_descriptors(FIELD_ADVECT_PRESET_ID);
        let speed_desc = descriptors
            .iter()
            .find(|d| d.key == "flow_speed")
            .expect("flow_speed descriptor must be present");
        assert_eq!(speed_desc.default, 0.0, "flow_speed identity default = 0.0");
    }

    // ----- PCleanup.1.2 — fluid_warp treatment ---------------------------

    #[test]
    fn fluid_warp_is_registered() {
        assert!(
            is_registered(FLUID_WARP_PRESET_ID),
            "fluid_warp must appear in treatments::registry()"
        );
    }

    #[test]
    fn fluid_warp_descriptors_present() {
        let descriptors = param_descriptors(FLUID_WARP_PRESET_ID);
        assert_eq!(descriptors.len(), 1, "fluid_warp exposes exactly 1 param");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn fluid_warp_amplitude_default_is_zero() {
        let descriptors = param_descriptors(FLUID_WARP_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(
            amplitude_desc.default, 0.0,
            "amplitude identity default = 0.0"
        );
    }

    // ----- PCleanup.2.3 — fluid_warp_full treatment -----------------------

    #[test]
    fn fluid_warp_full_is_registered() {
        assert!(
            is_registered(FLUID_WARP_FULL_PRESET_ID),
            "fluid_warp_full must appear in treatments::registry()"
        );
    }

    #[test]
    fn fluid_warp_full_descriptors_present() {
        let descriptors = param_descriptors(FLUID_WARP_FULL_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            1,
            "fluid_warp_full exposes exactly 1 param"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn fluid_warp_full_amplitude_default_is_zero() {
        let descriptors = param_descriptors(FLUID_WARP_FULL_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(
            amplitude_desc.default, 0.0,
            "amplitude identity default = 0.0"
        );
    }

    // ----- PCleanup.2.9 — zone_brighten treatment ------------------------

    #[test]
    fn zone_brighten_is_registered() {
        assert!(
            is_registered(ZONE_BRIGHTEN_PRESET_ID),
            "zone_brighten must appear in treatments::registry()"
        );
    }

    #[test]
    fn zone_brighten_descriptors_present() {
        let descriptors = param_descriptors(ZONE_BRIGHTEN_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            4,
            "zone_brighten exposes exactly 4 params"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn zone_brighten_intensity_default_is_zero() {
        let descriptors = param_descriptors(ZONE_BRIGHTEN_PRESET_ID);
        let intensity_desc = descriptors
            .iter()
            .find(|d| d.key == "intensity")
            .expect("intensity descriptor must be present");
        assert_eq!(
            intensity_desc.default, 0.0,
            "intensity identity default = 0.0"
        );
    }

    // ----- PCleanup.2.10 — zone_lens treatment ---------------------------

    #[test]
    fn zone_lens_is_registered() {
        assert!(
            is_registered(ZONE_LENS_PRESET_ID),
            "zone_lens must appear in treatments::registry()"
        );
    }

    #[test]
    fn zone_lens_descriptors_present() {
        let descriptors = param_descriptors(ZONE_LENS_PRESET_ID);
        assert_eq!(descriptors.len(), 4, "zone_lens exposes exactly 4 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn zone_lens_amplitude_default_is_zero() {
        let descriptors = param_descriptors(ZONE_LENS_PRESET_ID);
        let amplitude_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(
            amplitude_desc.default, 0.0,
            "amplitude identity default = 0.0"
        );
    }

    // ----- PCleanup.2.4 — spotlights treatment ---------------------------

    #[test]
    fn spotlights_is_registered() {
        assert!(
            is_registered(SPOTLIGHTS_PRESET_ID),
            "spotlights must appear in treatments::registry()"
        );
    }

    #[test]
    fn spotlights_descriptors_present() {
        let descriptors = param_descriptors(SPOTLIGHTS_PRESET_ID);
        assert_eq!(descriptors.len(), 4, "spotlights exposes exactly 4 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn spotlights_brightness_gain_default_is_zero() {
        let descriptors = param_descriptors(SPOTLIGHTS_PRESET_ID);
        let gain_desc = descriptors
            .iter()
            .find(|d| d.key == "brightness_gain")
            .expect("brightness_gain descriptor must be present");
        assert_eq!(
            gain_desc.default, 0.0,
            "brightness_gain identity default = 0.0"
        );
    }

    // ----- PCleanup.2.5a — drift_pinholes treatment ----------------------

    #[test]
    fn drift_pinholes_is_registered() {
        assert!(
            is_registered(DRIFT_PINHOLES_PRESET_ID),
            "drift_pinholes must appear in treatments::registry()"
        );
    }

    #[test]
    fn drift_pinholes_descriptors_present() {
        let descriptors = param_descriptors(DRIFT_PINHOLES_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            4,
            "drift_pinholes exposes exactly 4 params"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn drift_pinholes_opacity_default_is_zero() {
        let descriptors = param_descriptors(DRIFT_PINHOLES_PRESET_ID);
        let opacity_desc = descriptors
            .iter()
            .find(|d| d.key == "opacity")
            .expect("opacity descriptor must be present");
        assert_eq!(opacity_desc.default, 0.0, "opacity identity default = 0.0");
    }

    /// PCleanup.2.5a — drift_pinholes is classified as a source-modifying
    /// Treatment (it masks the source by particle proximity).
    #[test]
    fn drift_pinholes_is_source_modifier_group() {
        assert_eq!(
            treatment_group(DRIFT_PINHOLES_PRESET_ID),
            TreatmentGroup::SourceModifier
        );
    }

    // ----- PCleanup.2.5b — drift_brushstrokes treatment ------------------

    #[test]
    fn drift_brushstrokes_is_registered() {
        assert!(
            is_registered(DRIFT_BRUSHSTROKES_PRESET_ID),
            "drift_brushstrokes must appear in treatments::registry()"
        );
    }

    #[test]
    fn drift_brushstrokes_descriptors_present() {
        let descriptors = param_descriptors(DRIFT_BRUSHSTROKES_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            5,
            "drift_brushstrokes exposes 5 params (adds smear_duration over drift_pinholes)"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn drift_brushstrokes_opacity_default_is_zero() {
        let descriptors = param_descriptors(DRIFT_BRUSHSTROKES_PRESET_ID);
        let opacity_desc = descriptors
            .iter()
            .find(|d| d.key == "opacity")
            .expect("opacity descriptor must be present");
        assert_eq!(opacity_desc.default, 0.0, "opacity identity default = 0.0");
    }

    #[test]
    fn drift_brushstrokes_has_smear_duration() {
        let descriptors = param_descriptors(DRIFT_BRUSHSTROKES_PRESET_ID);
        assert!(
            descriptors.iter().any(|d| d.key == "smear_duration"),
            "drift_brushstrokes must expose smear_duration"
        );
    }

    #[test]
    fn drift_brushstrokes_is_source_modifier_group() {
        assert_eq!(
            treatment_group(DRIFT_BRUSHSTROKES_PRESET_ID),
            TreatmentGroup::SourceModifier
        );
    }

    // ----- PCleanup.2.6 — edge_sparks treatment --------------------------

    #[test]
    fn edge_sparks_is_registered() {
        assert!(
            is_registered(EDGE_SPARKS_PRESET_ID),
            "edge_sparks must appear in treatments::registry()"
        );
    }

    #[test]
    fn edge_sparks_descriptors_present() {
        let descriptors = param_descriptors(EDGE_SPARKS_PRESET_ID);
        assert_eq!(
            descriptors.len(),
            5,
            "edge_sparks exposes 5 params (adds lifetime_s over spotlights)"
        );
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn edge_sparks_brightness_gain_default_is_zero() {
        let descriptors = param_descriptors(EDGE_SPARKS_PRESET_ID);
        let gain_desc = descriptors
            .iter()
            .find(|d| d.key == "brightness_gain")
            .expect("brightness_gain descriptor must be present");
        assert_eq!(
            gain_desc.default, 0.0,
            "brightness_gain identity default = 0.0"
        );
    }

    #[test]
    fn edge_sparks_has_lifetime() {
        let descriptors = param_descriptors(EDGE_SPARKS_PRESET_ID);
        assert!(
            descriptors.iter().any(|d| d.key == "lifetime_s"),
            "edge_sparks must expose lifetime_s"
        );
    }

    #[test]
    fn edge_sparks_is_source_modifier_group() {
        assert_eq!(
            treatment_group(EDGE_SPARKS_PRESET_ID),
            TreatmentGroup::SourceModifier
        );
    }

    // ----- PCleanup.2.8 — collision_ripples treatment --------------------

    #[test]
    fn collision_ripples_is_registered() {
        assert!(
            is_registered(COLLISION_RIPPLES_PRESET_ID),
            "collision_ripples must appear in treatments::registry()"
        );
    }

    #[test]
    fn collision_ripples_descriptors_present() {
        let descriptors = param_descriptors(COLLISION_RIPPLES_PRESET_ID);
        assert_eq!(descriptors.len(), 7, "collision_ripples exposes 7 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn collision_ripples_amplitude_default_is_zero() {
        let descriptors = param_descriptors(COLLISION_RIPPLES_PRESET_ID);
        let amp_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(amp_desc.default, 0.0, "amplitude identity default = 0.0");
    }

    #[test]
    fn collision_ripples_is_source_modifier_group() {
        assert_eq!(
            treatment_group(COLLISION_RIPPLES_PRESET_ID),
            TreatmentGroup::SourceModifier
        );
    }

    // ----- PCleanup.2.11 — portal_warp treatment -------------------------

    #[test]
    fn portal_warp_is_registered() {
        assert!(
            is_registered(PORTAL_WARP_PRESET_ID),
            "portal_warp must appear in treatments::registry()"
        );
    }

    #[test]
    fn portal_warp_descriptors_present() {
        let descriptors = param_descriptors(PORTAL_WARP_PRESET_ID);
        assert_eq!(descriptors.len(), 5, "portal_warp exposes 5 params");
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    #[test]
    fn portal_warp_amplitude_default_is_zero() {
        let descriptors = param_descriptors(PORTAL_WARP_PRESET_ID);
        let amp_desc = descriptors
            .iter()
            .find(|d| d.key == "amplitude")
            .expect("amplitude descriptor must be present");
        assert_eq!(amp_desc.default, 0.0, "amplitude identity default = 0.0");
    }

    #[test]
    fn portal_warp_is_source_modifier_group() {
        assert_eq!(
            treatment_group(PORTAL_WARP_PRESET_ID),
            TreatmentGroup::SourceModifier
        );
    }

    // ----- T1.20 — PresetCapability tests -----------------------------------

    /// Every preset in registry() must have a non-zero-value PresetCapability
    /// (at least one of requires_sdf/requires_zone/is_particle is true, OR
    /// headline_param is Some), except identity which is deliberately all-default.
    #[test]
    fn capability_every_registered_preset_is_non_default_except_identity() {
        let zero_value = PresetCapability {
            requires_sdf: false,
            requires_zone: false,
            is_particle: false,
            headline_param: None,
        };
        for (id, _label) in registry() {
            let cap = capability(id);
            if *id == IDENTITY_PRESET_ID {
                // Identity is the one allowed all-default exception.
                assert_eq!(
                    cap, zero_value,
                    "identity must return all-default PresetCapability"
                );
            } else {
                assert!(
                    cap.requires_sdf
                        || cap.requires_zone
                        || cap.is_particle
                        || cap.headline_param.is_some(),
                    "preset {id} must have at least one non-default capability field"
                );
            }
        }
    }

    /// Zone-keyed presets set both requires_sdf and requires_zone.
    #[test]
    fn capability_zone_presets_require_both_sdf_and_zone() {
        for id in &[ZONE_BRIGHTEN_PRESET_ID, ZONE_LENS_PRESET_ID] {
            let cap = capability(id);
            assert!(cap.requires_sdf, "{id}: requires_sdf must be true");
            assert!(cap.requires_zone, "{id}: requires_zone must be true");
        }
    }

    /// Particle presets are flagged is_particle.
    #[test]
    fn capability_particle_presets_are_flagged() {
        for id in &[
            SPOTLIGHTS_PRESET_ID,
            DRIFT_PINHOLES_PRESET_ID,
            DRIFT_BRUSHSTROKES_PRESET_ID,
            EDGE_SPARKS_PRESET_ID,
            COLLISION_RIPPLES_PRESET_ID,
        ] {
            let cap = capability(id);
            assert!(cap.is_particle, "{id}: is_particle must be true");
        }
    }

    /// blur_mask headline_param is the actual descriptor key (max_radius_px).
    #[test]
    fn capability_blur_mask_headline_is_max_radius_px() {
        let cap = capability(BLUR_MASK_PRESET_ID);
        assert_eq!(cap.headline_param, Some("max_radius_px"));
    }

    // ----- T1.21 — treatment_is_no_op tests ---------------------------------

    /// Positive case: ripple_lens with a non-zero amplitude and a mask polygon
    /// returns None (not a no-op).
    #[test]
    fn no_op_healthy_ripple_lens_returns_none() {
        let mut layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        layer.warp.mask_polygon = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut params = HashMap::new();
        params.insert("amplitude".to_string(), 0.05_f32);
        assert_eq!(
            treatment_is_no_op(RIPPLE_LENS_PRESET_ID, &params, &layer),
            None
        );
    }

    /// SDF-keyed preset with empty mask_polygon reports "Needs a mask polygon".
    #[test]
    fn no_op_sdf_preset_empty_mask_polygon() {
        let layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        // WarpMesh::identity() has an empty mask_polygon by default.
        assert!(layer.warp.mask_polygon.is_empty());
        let params = HashMap::new();
        assert_eq!(
            treatment_is_no_op(RIPPLE_LENS_PRESET_ID, &params, &layer),
            Some("Needs a mask polygon")
        );
    }

    /// Zone-keyed preset with no zone_role reports "Needs a zone role"
    /// (given that the mask_polygon is populated so requires_sdf passes first).
    #[test]
    fn no_op_zone_preset_no_zone_role() {
        let mut layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        layer.warp.mask_polygon = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        layer.warp.zone_role = None;
        let params = HashMap::new();
        assert_eq!(
            treatment_is_no_op(ZONE_BRIGHTEN_PRESET_ID, &params, &layer),
            Some("Needs a zone role")
        );
    }

    /// ripple_lens with amplitude ≈ 0 (and a mask) reports "Amplitude at 0".
    #[test]
    fn no_op_ripple_lens_amplitude_zero() {
        let mut layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        layer.warp.mask_polygon = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut params = HashMap::new();
        params.insert("amplitude".to_string(), 0.0_f32);
        assert_eq!(
            treatment_is_no_op(RIPPLE_LENS_PRESET_ID, &params, &layer),
            Some("Amplitude at 0")
        );
    }

    /// tone_map at all-identity params reports "All params at identity".
    #[test]
    fn no_op_tone_map_identity_params() {
        let layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        // Default params: exposure=0, contrast=1, shoulder=0 → identity.
        let params = HashMap::new();
        assert_eq!(
            treatment_is_no_op(TONE_MAP_PRESET_ID, &params, &layer),
            Some("All params at identity")
        );
    }

    /// tone_map with a non-identity param is not flagged as no-op.
    #[test]
    fn no_op_tone_map_non_identity_returns_none() {
        let layer =
            crate::project::schema::layer_from_image_path("t", std::path::PathBuf::from("x.png"));
        let mut params = HashMap::new();
        params.insert("exposure".to_string(), 0.5_f32);
        assert_eq!(
            treatment_is_no_op(TONE_MAP_PRESET_ID, &params, &layer),
            None
        );
    }
}
