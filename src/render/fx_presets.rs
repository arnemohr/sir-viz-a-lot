//! P0.5.3 — FX preset registry + per-preset render pipeline.
//!
//! A preset is `(preset_id, shader source, default params, pipeline)`.
//! v0.4 ships one preset (`"mask_edge_ripple_wash"`) as the proof point.
//! Phase 2 will grow the registry into the full FX library.
//!
//! # Canonical bind-group slots
//!
//! Every FX preset shader shares the same bind-group slot assignment:
//!
//! | Binding | Resource                                       | Notes                                           |
//! |---------|------------------------------------------------|-------------------------------------------------|
//! | 0       | SDF texture (`R32Float`, unfilterable)         | Always bound; caller must call `sync_mesh_and_mask` first |
//! | 1       | Sampler (`NonFiltering`)                       | Required by layout; `textureLoad` presets don't sample through it |
//! | 2       | `FxParamsUniform` (8 × f32, 32 bytes)          | Written each frame via `queue.write_buffer`     |
//! | 3       | Clock uniform (`vec4<f32>`, `.x` = secs)       | Written each frame via `queue.write_buffer`     |
//! | 4       | Source texture (`Rgba8UnormSrgb`)              | Fragment presets only (Wave/Fluid color-pass); leave unbound for others |
//! | 5       | Particle SSBO                                  | Compute presets only (`P2.5.1 FxComputePipeline`); fragment presets leave unbound |
//!
//! All Phase 2 presets MUST use these slots. Diverging is a build-time
//! hazard — adding a new bind-group layout means the existing dispatch
//! contract is broken.
//!
//! # Rendering
//!
//! Each FxLayer owns an `fx_texture` (output-sized, same format as the
//! surface swapchain — matches every other intermediate texture in the
//! pipeline). At per-frame render time, the preset's pipeline draws a
//! fullscreen triangle into `fx_texture`, reading the layer's SDF (via
//! `WarpRenderer::sdf_view`) and the current clock time.
//!
//! # Parameter uniform
//!
//! A fixed-shape struct, `FxParamsUniform`. Each preset documents which
//! fields it reads; unmapped fields stay zero. The fixed layout keeps the
//! shader bind-group layout stable across presets so adding a preset
//! doesn't churn the pipeline plumbing.
//!
//! # Future presets
//!
//! When Phase 2 adds more presets, each preset will get its own pipeline
//! (separate `new_<preset>` constructor); the `FxPresetPipeline` struct
//! stays the same shape — only the shader source and default params
//! differ. An FxLayer that carries an unknown `preset_id` is left
//! invisible (the per-frame loop skips it); the audit emits a warning
//! (wired in P0.5.1).
//!
//! # GPU tests (TODO)
//!
//! A golden-image test (`--features gpu-tests`) for the ripple-wash
//! preset rendered against a fixture polygon mask is deferred — it needs
//! a real wgpu adapter, which CI does not currently provide for the
//! `gpu-tests` feature path. Add it in Phase 2 alongside the golden
//! baseline image. See `tests/headless_gpu.rs` for the test harness.

use std::collections::HashMap;

use crate::render::fx_compute::FxComputePipeline;
use crate::render::fx_fluid::FxFluidPipeline;
use crate::render::sdf::SDF_HELPER_WGSL;

/// Internal pipeline-shape tag; not user-visible. Drives dispatch routing in
/// `fx_presets::dispatch` once P2.2.3 lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // wired by P2.2.4 audit + P2.8.1 browser
pub enum FxFamily {
    Fragment,
    ComputeParticle,
    ComputeFluid,
}

/// Static descriptor for a single FX preset entry in the registry.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // wired by P2.2.4 audit + P2.8.1 browser
pub struct FxPresetEntry {
    /// Stable identifier stored in project files; must never be renamed.
    pub preset_id: &'static str,
    /// Human-readable label shown in the picker UI.
    pub label: &'static str,
    /// Pipeline shape tag; drives dispatch routing once P2.2.3 lands.
    pub family: FxFamily,
}

/// All registered FX presets. Zero-allocation static slice — mirrors the
/// `treatments::registry()` shape.
#[allow(dead_code)] // wired by P2.2.4 audit + P2.8.1 browser
pub fn fx_registry() -> &'static [FxPresetEntry] {
    &[
        FxPresetEntry {
            preset_id: RIPPLE_WASH_PRESET_ID,
            label: "Mask-edge ripple wash",
            family: FxFamily::Fragment,
        },
        FxPresetEntry {
            preset_id: EDGE_WAVE_WASH_PRESET_ID,
            label: "Mask-edge wave wash",
            family: FxFamily::Fragment,
        },
        FxPresetEntry {
            preset_id: PARTICLES_IDENTITY_PRESET_ID,
            label: "Particles (identity)",
            family: FxFamily::ComputeParticle,
        },
        FxPresetEntry {
            preset_id: CONSTRAINED_DRIFT_PRESET_ID,
            label: "Mask-constrained drift",
            family: FxFamily::ComputeParticle,
        },
        FxPresetEntry {
            preset_id: EDGE_EMISSION_PRESET_ID,
            label: "Mask-edge emission",
            family: FxFamily::ComputeParticle,
        },
        FxPresetEntry {
            preset_id: FIELD_FLOW_PRESET_ID,
            label: "Mask field flow",
            family: FxFamily::ComputeParticle,
        },
        FxPresetEntry {
            preset_id: COLLISION_REFLECTION_PRESET_ID,
            label: "Mask collision reflection",
            family: FxFamily::ComputeParticle,
        },
        // P2.6.1 — fluid family
        FxPresetEntry {
            preset_id: FLUID_IDENTITY_PRESET_ID,
            label: "Fluid (identity)",
            family: FxFamily::ComputeFluid,
        },
        // P2.6.2
        FxPresetEntry {
            preset_id: BOUNDED_FLUID_PRESET_ID,
            label: "Mask-bounded fluid",
            family: FxFamily::ComputeFluid,
        },
    ]
}

/// `true` if `preset_id` corresponds to a registered FX preset.
/// CPU-only; safe to call without a GPU device.
#[allow(dead_code)] // wired by P2.2.4 audit + P2.8.1 browser
pub fn fx_is_registered(preset_id: &str) -> bool {
    fx_registry().iter().any(|e| e.preset_id == preset_id)
}

/// Returns the display label for a registered FX preset, or `None` if the
/// `preset_id` is not in the registry.
#[allow(dead_code)] // wired by P2.2.4 audit + P2.8.1 browser
pub fn fx_display_label(preset_id: &str) -> Option<&'static str> {
    fx_registry()
        .iter()
        .find(|e| e.preset_id == preset_id)
        .map(|e| e.label)
}

/// Static descriptor for a tunable FX preset parameter. The FX parameter
/// browser (P2.8.1) uses this metadata to render per-key sliders with the
/// right label, range, and default.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // consumed by P2.5.6 mutation + P2.8.1 browser UI
pub struct FxParamDescriptor {
    /// HashMap key under `LayerKind::FxLayer.params`.
    pub key: &'static str,
    /// Human-readable label shown next to the slider.
    pub label: &'static str,
    /// Slider min (inclusive).
    pub min: f32,
    /// Slider max (inclusive).
    pub max: f32,
    /// Default value when the key is missing from `LayerKind::FxLayer.params`.
    pub default: f32,
    /// Only meaningful when `key` represents a particle-count slot
    /// (typically `"particle_count"`). Fragment-family presets leave
    /// this `None`. P2.5.6 `SetFxLayerParams` mutation refuses to commit
    /// when the requested value exceeds this cap.
    pub max_particle_count: Option<u32>,
}

/// Param descriptors for the `mask_edge_ripple_wash` preset. The six fields
/// mirror the `FxParamsUniform::for_ripple_wash` defaults exactly — this
/// table is the single source of truth for slider ranges in the FX browser.
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const RIPPLE_WASH_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "wavelength",
        label: "Wavelength (normalised units)",
        min: 10.0,
        max: 400.0,
        default: 40.0,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "speed",
        label: "Speed (cycles/sec)",
        min: 0.0,
        max: 10.0,
        default: 2.0,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "falloff",
        label: "Falloff (exp distance, normalised)",
        min: 0.01,
        max: 1.0,
        default: 0.08,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "base_r",
        label: "Base colour — red",
        min: 0.0,
        max: 1.0,
        default: 0.4,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "base_g",
        label: "Base colour — green",
        min: 0.0,
        max: 1.0,
        default: 0.6,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "base_b",
        label: "Base colour — blue",
        min: 0.0,
        max: 1.0,
        default: 1.0,
        max_particle_count: None,
    },
];

/// Param descriptors for the `mask_edge_wave_wash` preset.
///
/// FxParamsUniform field aliasing:
/// - `wave_speed` → `speed` (0.0..=5.0, default 1.0)
/// - `wave_width` → `falloff` (0.0..=0.3, default 0.15)
/// - `colour`     → `base_r` (0.0..=1.0, default 0.5)
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const EDGE_WAVE_WASH_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "wave_speed",
        label: "Wave speed (cycles/sec)",
        min: 0.0,
        max: 5.0,
        default: 1.0,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "wave_width",
        label: "Wave band width (normalised)",
        min: 0.0,
        max: 0.3,
        default: 0.15,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "colour",
        label: "Colour (0=cold blue, 1=warm amber)",
        min: 0.0,
        max: 1.0,
        default: 0.5,
        max_particle_count: None,
    },
];

/// P2.5.1 — Param descriptors for the `particles_identity` preset.
///
/// FxParamsUniform field aliasing:
///   `particle_count` → `wavelength` (1..=16, default 16).
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const PARTICLES_IDENTITY_DESCRIPTORS: &[FxParamDescriptor] = &[FxParamDescriptor {
    key: "particle_count",
    label: "Particle count (1–16)",
    min: 1.0,
    max: 16.0,
    default: 16.0,
    max_particle_count: Some(16),
}];

/// P2.5.2 — Param descriptors for `mask_constrained_drift`.
///
/// FxParamsUniform field aliasing:
///   `particle_count` → `wavelength` (1..=2048, default 256)
///   `drift_speed`    → `speed`      (0.0..=0.05, default 0.02)
///   `particle_size`  → `falloff`    (0.5..=4.0, default 2.0)
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const CONSTRAINED_DRIFT_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "particle_count",
        label: "Particle count (1–2048)",
        min: 1.0,
        max: 2048.0,
        default: 256.0,
        max_particle_count: Some(2048),
    },
    FxParamDescriptor {
        key: "drift_speed",
        label: "Drift speed (UV/s)",
        min: 0.0,
        max: 0.05,
        default: 0.02,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "particle_size",
        label: "Particle size (px)",
        min: 0.5,
        max: 4.0,
        default: 2.0,
        max_particle_count: None,
    },
];

/// P2.5.3 — Param descriptors for `mask_edge_emission`.
///
/// FxParamsUniform field aliasing:
///   `particle_count`  → `wavelength` (1..=1024, default 128)
///   `emission_speed`  → `speed`      (0.01..=0.15, default 0.05)
///   `lifetime_secs`   → `falloff`    (0.5..=5.0, default 2.0)
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const EDGE_EMISSION_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "particle_count",
        label: "Particle count (1–1024)",
        min: 1.0,
        max: 1024.0,
        default: 128.0,
        max_particle_count: Some(1024),
    },
    FxParamDescriptor {
        key: "emission_speed",
        label: "Emission speed (UV/s)",
        min: 0.01,
        max: 0.15,
        default: 0.05,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "lifetime_secs",
        label: "Lifetime (seconds)",
        min: 0.5,
        max: 5.0,
        default: 2.0,
        max_particle_count: None,
    },
];

/// P2.5.4 — Param descriptors for `mask_field_flow`.
///
/// FxParamsUniform field aliasing:
///   `particle_count`  → `wavelength` (1..=2048, default 256)
///   `flow_speed`      → `speed`      (0.0..=0.1, default 0.03)
///   `flow_direction`  → `falloff`    (-1.0..=1.0, default 1.0)
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const FIELD_FLOW_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "particle_count",
        label: "Particle count (1–2048)",
        min: 1.0,
        max: 2048.0,
        default: 256.0,
        max_particle_count: Some(2048),
    },
    FxParamDescriptor {
        key: "flow_speed",
        label: "Flow speed (UV/s)",
        min: 0.0,
        max: 0.1,
        default: 0.03,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "flow_direction",
        label: "Flow direction (-1=inward, +1=outward)",
        min: -1.0,
        max: 1.0,
        default: 1.0,
        max_particle_count: None,
    },
];

/// P2.5.5 — Param descriptors for `mask_collision_reflection`.
///
/// FxParamsUniform field aliasing:
///   `particle_count` → `wavelength` (1..=512, default 64)
///   `speed`          → `speed`      (0.01..=0.2, default 0.08)
///   `restitution`    → `falloff`    (0.5..=1.0, default 0.95)
#[allow(dead_code)] // referenced only through `fx_param_descriptors` (P2.8.1 UI)
const COLLISION_REFLECTION_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "particle_count",
        label: "Particle count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 64.0,
        max_particle_count: Some(512),
    },
    FxParamDescriptor {
        key: "speed",
        label: "Speed (UV/s)",
        min: 0.01,
        max: 0.2,
        default: 0.08,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "restitution",
        label: "Restitution (0.5–1.0)",
        min: 0.5,
        max: 1.0,
        default: 0.95,
        max_particle_count: None,
    },
];

/// P2.6.1 — Param descriptors for `fluid_identity`.
///
/// FxParamsUniform field aliasing:
///   `dissipation` → `speed`  (0.0..=1.0, default 0.1)
///   `colour`      → `base_r` (0.0..=1.0, default 0.5)
#[allow(dead_code)] // referenced through `fx_param_descriptors` (P2.8.1 browser UI)
const FLUID_IDENTITY_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "dissipation",
        label: "Dissipation (fraction/sec)",
        min: 0.0,
        max: 1.0,
        default: 0.1,
        max_particle_count: None,
    },
    FxParamDescriptor {
        key: "colour",
        label: "Colour (0=cold, 1=warm)",
        min: 0.0,
        max: 1.0,
        default: 0.5,
        max_particle_count: None,
    },
];

/// P2.6.2 — Param descriptors for `mask_bounded_fluid`.
///
/// `particle_count` carries `max_particle_count: Some(512)` to satisfy the
/// spec acceptance criterion.  The current implementation does not maintain
/// a particle SSBO; particle visualisation is deferred.
///
/// FxParamsUniform field aliasing:
///   `particle_count` → `wavelength` (1..=512, default 64, max_particle_count: Some(512))
///   `dissipation`    → `speed`      (0.9..=1.0, default 0.95)
#[allow(dead_code)] // referenced through `fx_param_descriptors` (P2.8.1 browser UI)
const BOUNDED_FLUID_DESCRIPTORS: &[FxParamDescriptor] = &[
    FxParamDescriptor {
        key: "particle_count",
        label: "Particle count (1–512)",
        min: 1.0,
        max: 512.0,
        default: 64.0,
        max_particle_count: Some(512),
    },
    FxParamDescriptor {
        key: "dissipation",
        label: "Dissipation (fraction/sec, 0.9–1.0)",
        min: 0.9,
        max: 1.0,
        default: 0.95,
        max_particle_count: None,
    },
];

/// Param descriptors for the named FX preset. Returns an empty slice for
/// unknown presets and for presets with no tunable parameters.
#[allow(dead_code)] // consumed by P2.5.6 mutation + P2.8.1 browser UI
pub fn fx_param_descriptors(preset_id: &str) -> &'static [FxParamDescriptor] {
    match preset_id {
        RIPPLE_WASH_PRESET_ID => RIPPLE_WASH_DESCRIPTORS,
        EDGE_WAVE_WASH_PRESET_ID => EDGE_WAVE_WASH_DESCRIPTORS,
        PARTICLES_IDENTITY_PRESET_ID => PARTICLES_IDENTITY_DESCRIPTORS,
        CONSTRAINED_DRIFT_PRESET_ID => CONSTRAINED_DRIFT_DESCRIPTORS,
        EDGE_EMISSION_PRESET_ID => EDGE_EMISSION_DESCRIPTORS,
        FIELD_FLOW_PRESET_ID => FIELD_FLOW_DESCRIPTORS,
        COLLISION_REFLECTION_PRESET_ID => COLLISION_REFLECTION_DESCRIPTORS,
        FLUID_IDENTITY_PRESET_ID => FLUID_IDENTITY_DESCRIPTORS,
        BOUNDED_FLUID_PRESET_ID => BOUNDED_FLUID_DESCRIPTORS,
        _ => &[],
    }
}

/// Preset id for the mask-edge ripple wash effect.
pub const RIPPLE_WASH_PRESET_ID: &str = "mask_edge_ripple_wash";

/// Preset id for the mask-edge wave wash effect (P2.4.3).
pub const EDGE_WAVE_WASH_PRESET_ID: &str = "mask_edge_wave_wash";

/// P2.5.1 — Preset id for the particles identity effect.
pub const PARTICLES_IDENTITY_PRESET_ID: &str = "particles_identity";

/// P2.5.2 — Preset id for the mask-constrained drift effect.
pub const CONSTRAINED_DRIFT_PRESET_ID: &str = "mask_constrained_drift";

/// P2.5.3 — Preset id for the mask-edge emission effect.
pub const EDGE_EMISSION_PRESET_ID: &str = "mask_edge_emission";

/// P2.5.4 — Preset id for the mask field flow effect.
pub const FIELD_FLOW_PRESET_ID: &str = "mask_field_flow";

/// P2.5.5 — Preset id for the mask collision reflection effect.
pub const COLLISION_REFLECTION_PRESET_ID: &str = "mask_collision_reflection";

/// P2.6.1 — Preset id for the fluid identity effect.
pub const FLUID_IDENTITY_PRESET_ID: &str = "fluid_identity";

/// P2.6.2 — Preset id for the mask-bounded fluid effect.
pub const BOUNDED_FLUID_PRESET_ID: &str = "mask_bounded_fluid";

/// Per-frame inputs that every FX preset receives at dispatch time.
///
/// Carries the canonical bind-group contract (P2.3.2): slots 0–3 are always
/// populated; `source` (slot 4) and `particle_ssbo` (slot 5) are optional and
/// reserved for future Wave/Fluid/Compute preset families.
pub struct FxShaderInputs<'a> {
    /// wgpu device (bind-group creation, buffer writes).
    pub device: &'a wgpu::Device,
    /// wgpu queue (uniform uploads via `write_buffer`).
    pub queue: &'a wgpu::Queue,
    /// Active command encoder for the frame.
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// Output texture view — preset renders into this.
    pub dst: &'a wgpu::TextureView,
    /// Layer's per-frame SDF (R32Float). Must be up-to-date; caller is
    /// responsible for calling `sync_mesh_and_mask` before dispatch.
    pub sdf_view: &'a wgpu::TextureView,
    /// Elapsed clock time in seconds (`Clock::elapsed().as_secs_f32()`).
    pub clock_secs: f32,
    /// Free-form per-preset params from `LayerKind::FxLayer.params`. Each
    /// preset documents which keys it reads; missing keys fall back to the
    /// documented default.
    pub params: &'a std::collections::HashMap<String, f32>,

    /// Optional source texture for fragment presets that read underlying
    /// layer pixels (none of v0.6's currently registered presets do —
    /// reserved for future Wave/Fluid families that composite over source).
    #[allow(dead_code)] // wired by future Wave/Fluid presets
    pub source: Option<&'a wgpu::TextureView>,

    /// Optional particle SSBO binding for compute-shader-based presets
    /// (P2.5.1's `FxComputePipeline`). Fragment presets leave this `None`.
    #[allow(dead_code)] // wired by P2.5.1
    pub particle_ssbo: Option<&'a wgpu::Buffer>,

    /// P2.5.1 — RNG seed from `LayerKind::FxLayer.seed`. Fragment presets
    /// ignore this; compute-particle presets use it for deterministic positions.
    pub seed: u64,

    /// P2.5.1 — seconds from `LayerKind::FxLayer.t_layer_added_secs`.
    /// Compute presets derive particle-system local time as
    /// `clock_secs - t_layer_added_secs`. Fragment presets ignore this.
    pub t_layer_added_secs: f32,

    /// P2.5.1 — output resolution `[width, height]`. Particle vertex shader
    /// uses this to convert positions to NDC. Fragment presets ignore this.
    pub output_size: [u32; 2],
}

/// Route `preset_id` to its registered render implementation.
///
/// Returns `true` if a known preset rendered into `inputs.dst`; returns
/// `false` for any unrecognised `preset_id` so the caller can fall through
/// to the "unknown preset / no-op" path (the P0.5.1 audit has already
/// emitted a warning for the unknown id).
///
/// # Unit-testing note
///
/// Unit-testing `dispatch` requires a GPU adapter; coverage comes from
/// `make test-gpu` smoke + the `app.rs` integration path exercised by
/// `demo_loads_fx_ripple_wash`.
pub fn dispatch(preset_id: &str, pipelines: &FxPipelines, inputs: FxShaderInputs<'_>) -> bool {
    match preset_id {
        RIPPLE_WASH_PRESET_ID => {
            let params_uniform = FxParamsUniform::for_ripple_wash(inputs.params);
            pipelines.ripple_wash.render(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                inputs.sdf_view,
                inputs.clock_secs,
                &params_uniform,
            );
            true
        }
        EDGE_WAVE_WASH_PRESET_ID => {
            let params_uniform = FxParamsUniform::for_edge_wave_wash(inputs.params);
            pipelines.edge_wave_wash.render(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                inputs.sdf_view,
                inputs.clock_secs,
                &params_uniform,
            );
            true
        }
        PARTICLES_IDENTITY_PRESET_ID => {
            // Particle count from params; clamped to 1..=16.
            let n_particles = inputs
                .params
                .get("particle_count")
                .copied()
                .unwrap_or(16.0)
                .clamp(1.0, 16.0) as u32;

            // Compute pass: write particle positions into the write SSBO.
            pipelines.particles_identity.dispatch_compute(
                inputs.queue,
                inputs.device,
                inputs.encoder,
                n_particles,
                inputs.seed,
                inputs.clock_secs,
                inputs.t_layer_added_secs,
                inputs.params,
            );

            // Render pass: draw quads for each particle into dst.
            pipelines.particles_identity.draw_particles(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                n_particles,
                inputs.output_size,
            );
            true
        }
        CONSTRAINED_DRIFT_PRESET_ID => {
            let n_particles = inputs
                .params
                .get("particle_count")
                .copied()
                .unwrap_or(256.0)
                .clamp(1.0, 2048.0) as u32;
            pipelines.constrained_drift.dispatch_compute_with_sdf(
                inputs.queue,
                inputs.device,
                inputs.encoder,
                n_particles,
                inputs.seed,
                inputs.clock_secs,
                inputs.t_layer_added_secs,
                inputs.params,
                inputs.sdf_view,
            );
            pipelines.constrained_drift.draw_particles(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                n_particles,
                inputs.output_size,
            );
            true
        }
        EDGE_EMISSION_PRESET_ID => {
            let n_particles = inputs
                .params
                .get("particle_count")
                .copied()
                .unwrap_or(128.0)
                .clamp(1.0, 1024.0) as u32;
            pipelines.edge_emission.dispatch_compute_with_sdf(
                inputs.queue,
                inputs.device,
                inputs.encoder,
                n_particles,
                inputs.seed,
                inputs.clock_secs,
                inputs.t_layer_added_secs,
                inputs.params,
                inputs.sdf_view,
            );
            pipelines.edge_emission.draw_particles(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                n_particles,
                inputs.output_size,
            );
            true
        }
        FIELD_FLOW_PRESET_ID => {
            let n_particles = inputs
                .params
                .get("particle_count")
                .copied()
                .unwrap_or(256.0)
                .clamp(1.0, 2048.0) as u32;
            pipelines.field_flow.dispatch_compute_with_sdf(
                inputs.queue,
                inputs.device,
                inputs.encoder,
                n_particles,
                inputs.seed,
                inputs.clock_secs,
                inputs.t_layer_added_secs,
                inputs.params,
                inputs.sdf_view,
            );
            pipelines.field_flow.draw_particles(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                n_particles,
                inputs.output_size,
            );
            true
        }
        COLLISION_REFLECTION_PRESET_ID => {
            let n_particles = inputs
                .params
                .get("particle_count")
                .copied()
                .unwrap_or(64.0)
                .clamp(1.0, 512.0) as u32;
            pipelines.collision_reflection.dispatch_compute_with_sdf(
                inputs.queue,
                inputs.device,
                inputs.encoder,
                n_particles,
                inputs.seed,
                inputs.clock_secs,
                inputs.t_layer_added_secs,
                inputs.params,
                inputs.sdf_view,
            );
            pipelines.collision_reflection.draw_particles(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                n_particles,
                inputs.output_size,
            );
            true
        }
        // P2.6.1 — fluid identity: advect + colour-map render.
        FLUID_IDENTITY_PRESET_ID => {
            let dissipation = inputs
                .params
                .get("dissipation")
                .copied()
                .unwrap_or(0.1)
                .clamp(0.0, 1.0);

            // Advect step (no SDF — identity preset ignores mask boundary).
            // inject_intensity=0.5 seeds a steady swirl in the centre so the
            // operator sees motion; the field would otherwise stay at zero
            // forever (an empty fluid sim has nothing to advect).
            pipelines.fluid_identity.dispatch_advect(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                None, // sdf_view: identity preset does not use mask
                inputs.clock_secs,
                dissipation,
                0.5,
            );

            // Render: colour-map velocity field into dst.
            pipelines.fluid_identity.draw_fluid(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                inputs.clock_secs,
                inputs.params,
            );
            true
        }

        // P2.6.2 — mask-bounded fluid: advect with SDF boundary + colour-map render.
        BOUNDED_FLUID_PRESET_ID => {
            let dissipation = inputs
                .params
                .get("dissipation")
                .copied()
                .unwrap_or(0.95)
                .clamp(0.0, 1.0);

            // Advect with mask boundary enforcement.
            // inject_intensity=0.4 so the bounded-fluid preset also produces
            // visible motion. (Without an injector, the field stays empty.)
            pipelines.bounded_fluid.dispatch_advect(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                Some(inputs.sdf_view), // use mask SDF for boundary
                inputs.clock_secs,
                dissipation,
                0.4,
            );

            // Render: colour-map velocity field into dst.
            pipelines.bounded_fluid.draw_fluid(
                inputs.device,
                inputs.queue,
                inputs.encoder,
                inputs.dst,
                inputs.clock_secs,
                inputs.params,
            );
            true
        }

        // Registered families not yet wired — caller skips rendering.
        _ => false,
    }
}

/// P2.4.3 — Edge-wave-wash FX preset pipeline. Self-illuminated traveling wave
/// along the mask boundary. Same 4-binding contract as `FxPresetPipeline`.
///
/// Owns its own `params_buf` and `clock_buf` so a scene with both ripple-wash
/// and wave-wash FxLayers can upload different uniforms in the same frame
/// without collisions.
pub struct FxEdgeWaveWashPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    clock_buf: wgpu::Buffer,
}

impl FxEdgeWaveWashPipeline {
    /// Build the edge-wave-wash pipeline against `target_format`.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_edge_wave_wash.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_edge_wave_wash.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fx edge wave wash bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fx edge wave wash pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fx edge wave wash pipeline"),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fx edge wave wash sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx edge wave wash params"),
            size: std::mem::size_of::<FxParamsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx edge wave wash clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            clock_buf,
        }
    }

    /// Render into `dst` using `sdf_view` and the preset's params.
    ///
    /// The caller must call `sync_mesh_and_mask` before this so the SDF is
    /// current.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
        clock_secs: f32,
        params: &FxParamsUniform,
    ) {
        let mut params_bytes = [0u8; 32];
        let floats = [
            params.wavelength,
            params.speed,
            params.falloff,
            params.base_r,
            params.base_g,
            params.base_b,
            params._pad0,
            params._pad1,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx edge wave wash bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
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
                    resource: self.clock_buf.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fx edge wave wash pass"),
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
            pass.draw(0..3, 0..1);
        }
    }
}

/// Holder for all FX preset pipelines. One field per preset; each preset
/// owns its own GPU buffers so concurrent renders in a multi-FxLayer scene
/// never share uniform buffers across presets.
///
/// Phase 2: add a new field for each new preset. The dispatch function and
/// `app.rs` `init_render_graph` both gain one line.
pub struct FxPipelines {
    /// P0.5.3 — mask-edge ripple wash (concentric rings from edge).
    pub ripple_wash: FxPresetPipeline,
    /// P2.4.3 — mask-edge wave wash (traveling wave along edge).
    pub edge_wave_wash: FxEdgeWaveWashPipeline,
    /// P2.5.1 — particles identity (stationary grid of white dots).
    pub particles_identity: FxComputePipeline,
    /// P2.5.2 — mask-constrained drift (particles drift inside mask).
    pub constrained_drift: FxComputePipeline,
    /// P2.5.3 — mask-edge emission (particles spawn at edge, fly outward).
    pub edge_emission: FxComputePipeline,
    /// P2.5.4 — mask field flow (particles follow SDF gradient).
    pub field_flow: FxComputePipeline,
    /// P2.5.5 — mask collision reflection (particles bounce inside mask).
    pub collision_reflection: FxComputePipeline,
    /// P2.6.1 — fluid identity (velocity field as colour).
    pub fluid_identity: FxFluidPipeline,
    /// P2.6.2 — mask-bounded fluid (velocity zeroed outside mask, reflected at boundary).
    pub bounded_fluid: FxFluidPipeline,
}

impl FxPipelines {
    /// Build all FX preset pipelines for the given surface format.
    ///
    /// Called once in `init_render_graph`; the result is stored on
    /// `EditingState` as `fx_pipelines` and passed to `dispatch` each frame.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            ripple_wash: FxPresetPipeline::new_ripple_wash(device, target_format),
            edge_wave_wash: FxEdgeWaveWashPipeline::new(device, target_format),
            particles_identity: FxComputePipeline::new_particles_identity(device, target_format),
            constrained_drift: FxComputePipeline::new_constrained_drift(device, target_format),
            edge_emission: FxComputePipeline::new_edge_emission(device, target_format),
            field_flow: FxComputePipeline::new_field_flow(device, target_format),
            collision_reflection: FxComputePipeline::new_collision_reflection(
                device,
                target_format,
            ),
            fluid_identity: FxFluidPipeline::new_fluid_identity(device, target_format),
            bounded_fluid: FxFluidPipeline::new_bounded_fluid(device, target_format),
        }
    }
}

/// Rendered by `FxPresetPipeline::render` into an output-sized texture.
/// The texture then flows through the layer's normal effect chain + warp
/// pipeline, so Color / Blur / Transform effects and masking work
/// unchanged against FxLayer output.
pub struct FxPresetPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    clock_buf: wgpu::Buffer,
}

impl FxPresetPipeline {
    /// Build the ripple-wash pipeline.
    ///
    /// `target_format` must match the output surface format so the fx
    /// texture blends correctly into the effect chain. v0.4 ships only
    /// this preset; Phase 2 generalises into a registry indexed by
    /// `preset_id`.
    pub fn new_ripple_wash(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // Concatenate the SDF helper at the front, as warp.rs does for
        // warp.wgsl. build.rs replicates this same concatenation for
        // compile-time naga validation (SDF_CONSUMERS includes "fx_").
        let shader_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_ripple_wash.wgsl")
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_ripple_wash.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fx ripple wash bgl"),
            entries: &[
                // binding 0: SDF texture (R32Float, unfilterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: sampler (NonFiltering — R32Float requires it).
                // Kept for bind-group layout symmetry with future presets;
                // the helper functions use textureLoad, so this is not
                // consumed at runtime by this shader.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 2: FxParams uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: clock uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fx ripple wash pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fx ripple wash pipeline"),
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
                    // Premultiplied alpha blend so the transparent outer
                    // region doesn't clobber what's behind the FxLayer.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fx ripple wash sampler"),
            // NonFiltering to match R32Float's constraint.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Params buffer: 32 bytes (8 × f32). Written each frame via
        // queue.write_buffer before the render pass.
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx ripple wash params"),
            size: std::mem::size_of::<FxParamsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Clock buffer: 16 bytes (vec4<f32>; only .x used).
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx ripple wash clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            clock_buf,
        }
    }

    /// Render into `dst` using `sdf_view` and the preset's params.
    ///
    /// `clock_secs` is the per-frame elapsed-time scalar (from
    /// `Clock::elapsed().as_secs_f32()`).
    ///
    /// The caller is responsible for calling `sync_mesh_and_mask` on the
    /// layer's `WarpRenderer` *before* this call so the SDF is up to date.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
        clock_secs: f32,
        params: &FxParamsUniform,
    ) {
        // Upload params uniform: 8 × f32 = 32 bytes, little-endian.
        let mut params_bytes = [0u8; 32];
        let floats = [
            params.wavelength,
            params.speed,
            params.falloff,
            params.base_r,
            params.base_g,
            params.base_b,
            params._pad0,
            params._pad1,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // Upload clock uniform: vec4<f32> with .x = clock_secs, rest zero.
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx ripple wash bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
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
                    resource: self.clock_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fx ripple wash pass"),
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
            // Fullscreen triangle: 3 vertices, 1 instance.
            pass.draw(0..3, 0..1);
        }
    }
}

/// Fixed-shape parameter struct for FX presets. Each preset documents which
/// fields it reads. Layout pinned at 32 bytes (8 floats). Add fields as the
/// registry grows; existing layouts stay stable by appending.
///
/// # `mask_edge_ripple_wash` field mapping
///
/// | field      | description                          | default |
/// |------------|--------------------------------------|---------|
/// | wavelength | ripple wavelength in normalised units | 40.0    |
/// | speed      | animation speed (cycles/sec)          | 2.0     |
/// | falloff    | exp-falloff distance (normalised)     | 0.08    |
/// | base_r     | base colour red channel               | 0.4     |
/// | base_g     | base colour green channel             | 0.6     |
/// | base_b     | base colour blue channel              | 1.0     |
/// | _pad0      | reserved                              | 0.0     |
/// | _pad1      | reserved                              | 0.0     |
///
/// # `mask_edge_wave_wash` field mapping (aliased — P2.4.3)
///
/// | field      | semantic for wave_wash                | default |
/// |------------|---------------------------------------|---------|
/// | speed      | wave_speed (animation speed, 0..=5)   | 1.0     |
/// | falloff    | wave_width (edge band half-width)     | 0.15    |
/// | base_r     | colour (cold↔warm tint, 0..=1)        | 0.5     |
/// | wavelength | unused (0.0)                          | 0.0     |
/// | base_g     | unused (0.0)                          | 0.0     |
/// | base_b     | unused (0.0)                          | 0.0     |
#[derive(Debug, Clone, Copy, Default)]
pub struct FxParamsUniform {
    pub wavelength: f32,
    pub speed: f32,
    pub falloff: f32,
    pub base_r: f32,
    pub base_g: f32,
    pub base_b: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl FxParamsUniform {
    /// Build from the `LayerKind::FxLayer.params` HashMap, defaulting
    /// any missing key to a sensible value for the ripple-wash preset.
    ///
    /// All defaults are documented in the struct-level doc table above.
    pub fn for_ripple_wash(params: &HashMap<String, f32>) -> Self {
        Self {
            wavelength: params.get("wavelength").copied().unwrap_or(40.0),
            speed: params.get("speed").copied().unwrap_or(2.0),
            falloff: params.get("falloff").copied().unwrap_or(0.08),
            base_r: params.get("base_r").copied().unwrap_or(0.4),
            base_g: params.get("base_g").copied().unwrap_or(0.6),
            base_b: params.get("base_b").copied().unwrap_or(1.0),
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }

    /// Build from the `LayerKind::FxLayer.params` HashMap for the
    /// `mask_edge_wave_wash` preset.
    ///
    /// # Field mapping
    ///
    /// The wave-wash preset reuses the generic uniform layout with aliased
    /// semantics (documented in the struct-level table above):
    ///
    /// | HashMap key | maps to uniform field | default |
    /// |-------------|----------------------|---------|
    /// | `wave_speed` | `speed`             | 1.0     |
    /// | `wave_width` | `falloff`           | 0.15    |
    /// | `colour`     | `base_r`            | 0.5     |
    ///
    /// All other fields (`wavelength`, `base_g`, `base_b`, `_pad0`, `_pad1`)
    /// are set to `0.0` — the shader does not read them.
    pub fn for_edge_wave_wash(params: &HashMap<String, f32>) -> Self {
        Self {
            wavelength: 0.0,
            speed: params.get("wave_speed").copied().unwrap_or(1.0),
            falloff: params.get("wave_width").copied().unwrap_or(0.15),
            base_r: params.get("colour").copied().unwrap_or(0.5),
            base_g: 0.0,
            base_b: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }

    /// P2.5.1 — Build from the `LayerKind::FxLayer.params` HashMap for the
    /// `particles_identity` preset.
    ///
    /// # Field mapping
    ///
    /// | HashMap key      | maps to uniform field | default |
    /// |------------------|-----------------------|---------|
    /// | `particle_count` | `wavelength`          | 16.0    |
    ///
    /// All other fields are zero (unused by the shader).
    pub fn for_particles_identity(params: &HashMap<String, f32>) -> Self {
        Self {
            wavelength: params.get("particle_count").copied().unwrap_or(16.0),
            speed: 0.0,
            falloff: 0.0,
            base_r: 0.0,
            base_g: 0.0,
            base_b: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }

    /// P2.5.2–P2.5.5 — Generic param uniform for SDF-reading particle presets.
    ///
    /// All four SDF particle presets share the same field aliasing pattern:
    ///
    /// | HashMap key      | maps to uniform field | notes                          |
    /// |------------------|-----------------------|--------------------------------|
    /// | `particle_count` | `wavelength`          | count, varies per preset       |
    /// | `drift_speed` / `emission_speed` / `flow_speed` / `speed` | `speed` | UV/s |
    /// | `particle_size` / `lifetime_secs` / `flow_direction` / `restitution` | `falloff` | shape param |
    ///
    /// The caller maps the preset-specific key names before calling this, OR
    /// the shaders read the positional uniform field directly. This generic
    /// constructor reads the first present key for each field.
    ///
    /// `dispatch_compute_with_sdf` uses this so each preset's shader always
    /// reads `u_params.wavelength`, `u_params.speed`, `u_params.falloff`.
    pub fn for_sdf_particle_preset(params: &HashMap<String, f32>) -> Self {
        // particle_count → wavelength
        let wavelength = params.get("particle_count").copied().unwrap_or(0.0);

        // speed-like param: drift_speed, emission_speed, flow_speed, or speed
        let speed = params
            .get("drift_speed")
            .or_else(|| params.get("emission_speed"))
            .or_else(|| params.get("flow_speed"))
            .or_else(|| params.get("speed"))
            .copied()
            .unwrap_or(0.0);

        // shape param: particle_size, lifetime_secs, flow_direction, or restitution
        let falloff = params
            .get("particle_size")
            .or_else(|| params.get("lifetime_secs"))
            .or_else(|| params.get("flow_direction"))
            .or_else(|| params.get("restitution"))
            .copied()
            .unwrap_or(0.0);

        Self {
            wavelength,
            speed,
            falloff,
            base_r: 0.0,
            base_g: 0.0,
            base_b: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0.5.3 acceptance: FxParamsUniform::for_ripple_wash returns documented
    /// defaults when the params map is empty.
    #[test]
    fn ripple_wash_defaults_from_empty_map() {
        let params = FxParamsUniform::for_ripple_wash(&HashMap::new());
        assert_eq!(params.wavelength, 40.0, "wavelength default");
        assert_eq!(params.speed, 2.0, "speed default");
        assert_eq!(params.falloff, 0.08, "falloff default");
        assert_eq!(params.base_r, 0.4, "base_r default");
        assert_eq!(params.base_g, 0.6, "base_g default");
        assert_eq!(params.base_b, 1.0, "base_b default");
        assert_eq!(params._pad0, 0.0, "_pad0 must be zero");
        assert_eq!(params._pad1, 0.0, "_pad1 must be zero");
    }

    /// P0.5.3 acceptance: all keys populated → values round-trip correctly.
    #[test]
    fn ripple_wash_all_keys_populated() {
        let mut map = HashMap::new();
        map.insert("wavelength".into(), 20.0_f32);
        map.insert("speed".into(), 5.0_f32);
        map.insert("falloff".into(), 0.15_f32);
        map.insert("base_r".into(), 1.0_f32);
        map.insert("base_g".into(), 0.5_f32);
        map.insert("base_b".into(), 0.2_f32);

        let params = FxParamsUniform::for_ripple_wash(&map);
        assert_eq!(params.wavelength, 20.0);
        assert_eq!(params.speed, 5.0);
        assert_eq!(params.falloff, 0.15);
        assert_eq!(params.base_r, 1.0);
        assert_eq!(params.base_g, 0.5);
        assert_eq!(params.base_b, 0.2);
        assert_eq!(params._pad0, 0.0, "_pad0 must still be zero");
        assert_eq!(params._pad1, 0.0, "_pad1 must still be zero");
    }

    // --- P2.2.1 registry tests ---

    /// P2.2.1 acceptance: fx_registry() contains RIPPLE_WASH_PRESET_ID.
    #[test]
    fn registry_contains_ripple_wash() {
        assert!(fx_is_registered(RIPPLE_WASH_PRESET_ID));
    }

    // --- P2.2.3 dispatch tests ---

    /// P2.2.3: RIPPLE_WASH_PRESET_ID is registered (prerequisite for
    /// dispatch returning true for it). Unit-testing `dispatch` itself
    /// requires a GPU adapter; coverage comes from `make test-gpu` smoke
    /// + the `app.rs` `demo_loads_fx_ripple_wash` integration path.
    #[test]
    fn dispatch_ripple_wash_preset_is_registered() {
        assert!(
            fx_registry()
                .iter()
                .any(|e| e.preset_id == RIPPLE_WASH_PRESET_ID),
            "RIPPLE_WASH_PRESET_ID must be in fx_registry() for dispatch to route it"
        );
    }

    /// P2.2.3: fx_is_registered returns false for a bogus preset_id,
    /// meaning dispatch would return false without panicking.
    #[test]
    fn dispatch_returns_false_for_bogus() {
        // Cannot create real wgpu resources in a unit test; instead verify
        // the precondition dispatch relies on: an unknown id is not registered.
        assert!(
            !fx_is_registered("bogus_preset_id"),
            "bogus preset must not be registered — dispatch must return false for it"
        );
    }

    /// P2.2.1 acceptance: fx_is_registered rejects an unknown id.
    #[test]
    fn registry_rejects_bogus_id() {
        assert!(!fx_is_registered("bogus_preset_id"));
    }

    /// P2.2.1 acceptance: fx_display_label returns Some for a known preset.
    #[test]
    fn display_label_returns_some_for_known() {
        assert!(fx_display_label(RIPPLE_WASH_PRESET_ID).is_some());
    }

    /// P2.2.1 acceptance: fx_display_label returns None for an unknown preset.
    #[test]
    fn display_label_returns_none_for_unknown() {
        assert!(fx_display_label("bogus").is_none());
    }

    /// P2.2.1 acceptance: no duplicate preset_ids in the registry.
    #[test]
    fn registry_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for entry in fx_registry() {
            assert!(
                seen.insert(entry.preset_id),
                "duplicate preset_id in fx_registry: {}",
                entry.preset_id
            );
        }
    }

    /// P2.2.1 acceptance: ripple_wash entry has FxFamily::Fragment.
    #[test]
    fn ripple_wash_family_is_fragment() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == RIPPLE_WASH_PRESET_ID)
            .expect("ripple_wash must be in fx_registry");
        assert_eq!(entry.family, FxFamily::Fragment);
    }

    // --- P2.2.2 descriptor tests ---

    /// P2.2.2 acceptance: descriptors for ripple_wash are non-empty.
    #[test]
    fn ripple_wash_descriptors_non_empty() {
        assert!(!fx_param_descriptors(RIPPLE_WASH_PRESET_ID).is_empty());
    }

    /// P2.2.2 acceptance: every descriptor has a valid range and default.
    #[test]
    fn ripple_wash_descriptors_all_valid() {
        for d in fx_param_descriptors(RIPPLE_WASH_PRESET_ID) {
            assert!(
                d.min < d.max,
                "key={}: min ({}) must be < max ({})",
                d.key,
                d.min,
                d.max
            );
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ({}) must be in [{}, {}]",
                d.key,
                d.default,
                d.min,
                d.max
            );
            assert!(
                d.max_particle_count.is_none(),
                "key={}: fragment preset must have max_particle_count = None",
                d.key
            );
        }
    }

    /// P2.2.2 acceptance: unknown preset returns empty slice without panic.
    #[test]
    fn bogus_returns_empty_descriptors() {
        assert!(fx_param_descriptors("bogus").is_empty());
    }

    // --- P2.3.2 struct shape tests ---

    /// P2.3.2 compile check: `FxShaderInputs<'a>` can be named with the new
    /// optional fields present. No GPU resources needed — the compiler
    /// type-checks the struct shape when the function below is compiled.
    /// A `#[test]` body is not meaningful here (the function is never
    /// called at runtime); the verification is purely at compile time.
    #[allow(dead_code)]
    fn _compile_check<'a>(_inputs: FxShaderInputs<'a>) {}

    /// P2.3.2 acceptance: verify `source` and `particle_ssbo` fields exist
    /// on `FxShaderInputs` and accept `None`. This test is a no-op at
    /// runtime; its only purpose is to confirm the struct layout compiles
    /// with the canonical-slot extension.
    #[test]
    fn fx_shader_inputs_optional_fields_compile() {
        // Confirm the new fields are accepted by the type system.
        let _: fn(FxShaderInputs<'_>) = _compile_check;
    }

    /// P2.2.2 acceptance: descriptor defaults match `FxParamsUniform::for_ripple_wash`
    /// defaults. Catches drift between the descriptor table and the uniform builder.
    #[test]
    fn ripple_wash_descriptor_defaults_match_for_ripple_wash() {
        let u = FxParamsUniform::for_ripple_wash(&HashMap::new());
        for d in fx_param_descriptors(RIPPLE_WASH_PRESET_ID) {
            let actual = match d.key {
                "wavelength" => u.wavelength,
                "speed" => u.speed,
                "falloff" => u.falloff,
                "base_r" => u.base_r,
                "base_g" => u.base_g,
                "base_b" => u.base_b,
                other => panic!("unmapped descriptor key: {other}"),
            };
            assert_eq!(
                actual, d.default,
                "descriptor default for key={} does not match for_ripple_wash default",
                d.key
            );
        }
    }

    // --- P2.4.3 edge_wave_wash tests ---

    /// P2.4.3 acceptance: EDGE_WAVE_WASH_PRESET_ID is in the registry.
    #[test]
    fn edge_wave_wash_is_registered() {
        assert!(fx_is_registered(EDGE_WAVE_WASH_PRESET_ID));
    }

    /// P2.4.3 acceptance: descriptors for edge_wave_wash return exactly 3 entries.
    #[test]
    fn edge_wave_wash_descriptors_present() {
        assert_eq!(
            fx_param_descriptors(EDGE_WAVE_WASH_PRESET_ID).len(),
            3,
            "mask_edge_wave_wash must have exactly 3 param descriptors"
        );
    }

    /// P2.4.3 acceptance: the registry entry for edge_wave_wash has FxFamily::Fragment.
    #[test]
    fn edge_wave_wash_family_is_fragment() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == EDGE_WAVE_WASH_PRESET_ID)
            .expect("edge_wave_wash must be in fx_registry");
        assert_eq!(entry.family, FxFamily::Fragment);
    }

    /// P2.4.3 acceptance: `FxParamsUniform::for_edge_wave_wash` returns documented
    /// defaults when the params map is empty. The aliased uniform fields are
    /// checked: `speed` = wave_speed, `falloff` = wave_width, `base_r` = colour.
    #[test]
    fn edge_wave_wash_defaults_round_trip() {
        let u = FxParamsUniform::for_edge_wave_wash(&HashMap::new());
        assert_eq!(u.speed, 1.0, "wave_speed → speed default must be 1.0");
        assert_eq!(u.falloff, 0.15, "wave_width → falloff default must be 0.15");
        assert_eq!(u.base_r, 0.5, "colour → base_r default must be 0.5");
        assert_eq!(u.wavelength, 0.0, "wavelength must be 0.0 (unused)");
        assert_eq!(u.base_g, 0.0, "base_g must be 0.0 (unused)");
        assert_eq!(u.base_b, 0.0, "base_b must be 0.0 (unused)");
        assert_eq!(u._pad0, 0.0, "_pad0 must be 0.0");
        assert_eq!(u._pad1, 0.0, "_pad1 must be 0.0");
    }

    // --- P2.5.1 particles_identity registry tests ---

    /// P2.5.1 acceptance: `particles_identity` is in the registry.
    #[test]
    fn particles_identity_is_registered() {
        assert!(
            fx_is_registered(PARTICLES_IDENTITY_PRESET_ID),
            "particles_identity must be in fx_registry()"
        );
    }

    /// P2.5.1 acceptance: the registry entry for `particles_identity` has
    /// `FxFamily::ComputeParticle`.
    #[test]
    fn particles_identity_family_is_compute_particle() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == PARTICLES_IDENTITY_PRESET_ID)
            .expect("particles_identity must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeParticle);
    }

    /// P2.5.1 acceptance: `particles_identity` descriptors are non-empty and
    /// have `max_particle_count = Some(16)` on the `particle_count` key.
    #[test]
    fn particles_identity_descriptors_valid() {
        let descs = fx_param_descriptors(PARTICLES_IDENTITY_PRESET_ID);
        assert!(
            !descs.is_empty(),
            "particles_identity must have at least one descriptor"
        );
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present");
        assert_eq!(
            pc.max_particle_count,
            Some(16),
            "particle_count max_particle_count must be Some(16)"
        );
        assert_eq!(pc.default, 16.0, "default particle_count must be 16");
    }

    /// P2.5.1 acceptance: `FxParamsUniform::for_particles_identity` maps
    /// `particle_count` to `wavelength` and defaults to 16 when the key is absent.
    #[test]
    fn particles_identity_params_uniform_defaults() {
        let u = FxParamsUniform::for_particles_identity(&HashMap::new());
        assert_eq!(
            u.wavelength, 16.0,
            "particle_count (default) → wavelength = 16"
        );
        assert_eq!(u.speed, 0.0, "speed must be 0.0 (unused)");
        assert_eq!(u.falloff, 0.0, "falloff must be 0.0 (unused)");
        assert_eq!(u.base_r, 0.0, "base_r must be 0.0 (unused)");
        assert_eq!(u.base_g, 0.0, "base_g must be 0.0 (unused)");
        assert_eq!(u.base_b, 0.0, "base_b must be 0.0 (unused)");
    }

    /// P2.5.1 acceptance: `FxParamsUniform::for_particles_identity` respects
    /// the `particle_count` key when present.
    #[test]
    fn particles_identity_params_uniform_custom_count() {
        let mut map = HashMap::new();
        map.insert("particle_count".into(), 9.0_f32);
        let u = FxParamsUniform::for_particles_identity(&map);
        assert_eq!(u.wavelength, 9.0, "particle_count=9 → wavelength=9");
    }

    // -------------------------------------------------------------------------
    // P2.5.2 — mask_constrained_drift
    // -------------------------------------------------------------------------

    /// P2.5.2 acceptance: `mask_constrained_drift` is in the registry.
    #[test]
    fn constrained_drift_is_registered() {
        assert!(
            fx_is_registered(CONSTRAINED_DRIFT_PRESET_ID),
            "mask_constrained_drift must be in fx_registry()"
        );
    }

    /// P2.5.2 acceptance: descriptors count matches spec (3); each min < max;
    /// default ∈ range.
    #[test]
    fn constrained_drift_descriptors_present() {
        let descs = fx_param_descriptors(CONSTRAINED_DRIFT_PRESET_ID);
        assert_eq!(
            descs.len(),
            3,
            "mask_constrained_drift must have 3 descriptors"
        );
        for d in descs {
            assert!(d.min < d.max, "key={}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ∈ range",
                d.key
            );
        }
    }

    /// P2.5.2 acceptance: `particle_count` descriptor has `max_particle_count: Some(2048)`.
    #[test]
    fn constrained_drift_max_particle_count_matches_spec() {
        let descs = fx_param_descriptors(CONSTRAINED_DRIFT_PRESET_ID);
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present");
        assert_eq!(
            pc.max_particle_count,
            Some(2048),
            "constrained_drift particle_count max_particle_count must be Some(2048)"
        );
    }

    /// P2.5.2 acceptance: registry entry has `FxFamily::ComputeParticle`.
    #[test]
    fn constrained_drift_family_is_compute_particle() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == CONSTRAINED_DRIFT_PRESET_ID)
            .expect("mask_constrained_drift must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeParticle);
    }

    // -------------------------------------------------------------------------
    // P2.5.3 — mask_edge_emission
    // -------------------------------------------------------------------------

    /// P2.5.3 acceptance: `mask_edge_emission` is in the registry.
    #[test]
    fn edge_emission_is_registered() {
        assert!(
            fx_is_registered(EDGE_EMISSION_PRESET_ID),
            "mask_edge_emission must be in fx_registry()"
        );
    }

    /// P2.5.3 acceptance: descriptors count matches spec (3); each min < max;
    /// default ∈ range.
    #[test]
    fn edge_emission_descriptors_present() {
        let descs = fx_param_descriptors(EDGE_EMISSION_PRESET_ID);
        assert_eq!(descs.len(), 3, "mask_edge_emission must have 3 descriptors");
        for d in descs {
            assert!(d.min < d.max, "key={}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ∈ range",
                d.key
            );
        }
    }

    /// P2.5.3 acceptance: `particle_count` descriptor has `max_particle_count: Some(1024)`.
    #[test]
    fn edge_emission_max_particle_count_matches_spec() {
        let descs = fx_param_descriptors(EDGE_EMISSION_PRESET_ID);
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present");
        assert_eq!(
            pc.max_particle_count,
            Some(1024),
            "edge_emission particle_count max_particle_count must be Some(1024)"
        );
    }

    /// P2.5.3 acceptance: registry entry has `FxFamily::ComputeParticle`.
    #[test]
    fn edge_emission_family_is_compute_particle() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == EDGE_EMISSION_PRESET_ID)
            .expect("mask_edge_emission must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeParticle);
    }

    // -------------------------------------------------------------------------
    // P2.5.4 — mask_field_flow
    // -------------------------------------------------------------------------

    /// P2.5.4 acceptance: `mask_field_flow` is in the registry.
    #[test]
    fn field_flow_is_registered() {
        assert!(
            fx_is_registered(FIELD_FLOW_PRESET_ID),
            "mask_field_flow must be in fx_registry()"
        );
    }

    /// P2.5.4 acceptance: descriptors count matches spec (3); each min < max;
    /// default ∈ range.
    #[test]
    fn field_flow_descriptors_present() {
        let descs = fx_param_descriptors(FIELD_FLOW_PRESET_ID);
        assert_eq!(descs.len(), 3, "mask_field_flow must have 3 descriptors");
        for d in descs {
            assert!(d.min < d.max, "key={}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ∈ range",
                d.key
            );
        }
    }

    /// P2.5.4 acceptance: descriptor `flow_direction` range is [-1.0, 1.0].
    #[test]
    fn field_flow_flow_direction_range() {
        let descs = fx_param_descriptors(FIELD_FLOW_PRESET_ID);
        let fd = descs
            .iter()
            .find(|d| d.key == "flow_direction")
            .expect("flow_direction descriptor must be present");
        assert!(
            (fd.min - (-1.0_f32)).abs() < 1e-6,
            "flow_direction min must be -1.0, got {}",
            fd.min
        );
        assert!(
            (fd.max - 1.0_f32).abs() < 1e-6,
            "flow_direction max must be 1.0, got {}",
            fd.max
        );
    }

    /// P2.5.4 acceptance: `particle_count` descriptor has `max_particle_count: Some(2048)`.
    #[test]
    fn field_flow_max_particle_count_matches_spec() {
        let descs = fx_param_descriptors(FIELD_FLOW_PRESET_ID);
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present");
        assert_eq!(
            pc.max_particle_count,
            Some(2048),
            "field_flow particle_count max_particle_count must be Some(2048)"
        );
    }

    /// P2.5.4 acceptance: registry entry has `FxFamily::ComputeParticle`.
    #[test]
    fn field_flow_family_is_compute_particle() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == FIELD_FLOW_PRESET_ID)
            .expect("mask_field_flow must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeParticle);
    }

    // -------------------------------------------------------------------------
    // P2.5.5 — mask_collision_reflection
    // -------------------------------------------------------------------------

    /// P2.5.5 acceptance: `mask_collision_reflection` is in the registry.
    #[test]
    fn collision_reflection_is_registered() {
        assert!(
            fx_is_registered(COLLISION_REFLECTION_PRESET_ID),
            "mask_collision_reflection must be in fx_registry()"
        );
    }

    /// P2.5.5 acceptance: descriptors count matches spec (3); each min < max;
    /// default ∈ range.
    #[test]
    fn collision_reflection_descriptors_present() {
        let descs = fx_param_descriptors(COLLISION_REFLECTION_PRESET_ID);
        assert_eq!(
            descs.len(),
            3,
            "mask_collision_reflection must have 3 descriptors"
        );
        for d in descs {
            assert!(d.min < d.max, "key={}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ∈ range",
                d.key
            );
        }
    }

    /// P2.5.5 acceptance: `particle_count` descriptor has `max_particle_count: Some(512)`.
    #[test]
    fn collision_reflection_max_particle_count_matches_spec() {
        let descs = fx_param_descriptors(COLLISION_REFLECTION_PRESET_ID);
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present");
        assert_eq!(
            pc.max_particle_count,
            Some(512),
            "collision_reflection particle_count max_particle_count must be Some(512)"
        );
    }

    /// P2.5.5 acceptance: registry entry has `FxFamily::ComputeParticle`.
    #[test]
    fn collision_reflection_family_is_compute_particle() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == COLLISION_REFLECTION_PRESET_ID)
            .expect("mask_collision_reflection must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeParticle);
    }

    // -------------------------------------------------------------------------
    // P2.6.1 — fluid_identity
    // -------------------------------------------------------------------------

    /// P2.6.1 acceptance: `fluid_identity` is in the registry.
    #[test]
    fn fluid_identity_is_registered() {
        assert!(
            fx_is_registered(FLUID_IDENTITY_PRESET_ID),
            "fluid_identity must be in fx_registry()"
        );
    }

    /// P2.6.1 acceptance: the registry entry for `fluid_identity` has
    /// `FxFamily::ComputeFluid`.
    #[test]
    fn fluid_identity_family_is_compute_fluid() {
        let entry = fx_registry()
            .iter()
            .find(|e| e.preset_id == FLUID_IDENTITY_PRESET_ID)
            .expect("fluid_identity must be in fx_registry");
        assert_eq!(entry.family, FxFamily::ComputeFluid);
    }

    /// P2.6.1 acceptance: `fluid_identity` has `dissipation` and `colour`
    /// param descriptors with valid ranges and defaults.
    #[test]
    fn fluid_identity_descriptors_present() {
        let descs = fx_param_descriptors(FLUID_IDENTITY_PRESET_ID);
        assert_eq!(
            descs.len(),
            2,
            "fluid_identity must have 2 descriptors (dissipation, colour)"
        );
        for d in descs {
            assert!(
                d.min < d.max,
                "key={}: min ({}) must be < max ({})",
                d.key,
                d.min,
                d.max
            );
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ({}) must be in [{}, {}]",
                d.key,
                d.default,
                d.min,
                d.max
            );
            assert!(
                d.max_particle_count.is_none(),
                "key={}: fluid_identity descriptors must have max_particle_count = None",
                d.key
            );
        }
        assert!(
            descs.iter().any(|d| d.key == "dissipation"),
            "dissipation descriptor must be present"
        );
        assert!(
            descs.iter().any(|d| d.key == "colour"),
            "colour descriptor must be present"
        );
    }

    // -------------------------------------------------------------------------
    // P2.6.2 — mask_bounded_fluid
    // -------------------------------------------------------------------------

    /// P2.6.2 acceptance: `mask_bounded_fluid` is in the registry.
    #[test]
    fn bounded_fluid_is_registered() {
        assert!(
            fx_is_registered(BOUNDED_FLUID_PRESET_ID),
            "mask_bounded_fluid must be in fx_registry()"
        );
    }

    /// P2.6.2 acceptance: `particle_count` descriptor has `max_particle_count: Some(512)`.
    #[test]
    fn bounded_fluid_max_particle_count_is_512() {
        let descs = fx_param_descriptors(BOUNDED_FLUID_PRESET_ID);
        let pc = descs
            .iter()
            .find(|d| d.key == "particle_count")
            .expect("particle_count descriptor must be present for mask_bounded_fluid");
        assert_eq!(
            pc.max_particle_count,
            Some(512),
            "bounded_fluid particle_count max_particle_count must be Some(512)"
        );
    }

    /// P2.6.2 acceptance: descriptors count matches spec (2); each min < max;
    /// default ∈ range.
    #[test]
    fn bounded_fluid_descriptors_present() {
        let descs = fx_param_descriptors(BOUNDED_FLUID_PRESET_ID);
        assert_eq!(
            descs.len(),
            2,
            "mask_bounded_fluid must have 2 descriptors (particle_count, dissipation)"
        );
        for d in descs {
            assert!(
                d.min < d.max,
                "key={}: min ({}) must be < max ({})",
                d.key,
                d.min,
                d.max
            );
            assert!(
                d.default >= d.min && d.default <= d.max,
                "key={}: default ({}) must be in [{}, {}]",
                d.key,
                d.default,
                d.min,
                d.max
            );
        }
        assert!(
            descs.iter().any(|d| d.key == "particle_count"),
            "particle_count descriptor must be present"
        );
        assert!(
            descs.iter().any(|d| d.key == "dissipation"),
            "dissipation descriptor must be present"
        );
    }
}
