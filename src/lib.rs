//! `rmap` library root — exposes crate modules for doctests and future
//! integration crates.
//!
//! The binary entry point lives in `src/main.rs`. This file exists so that
//! `cargo test --doc` can resolve crate items (e.g.
//! `rmap::project::command::ReverseStorage`) used in `compile_fail` doctests.

pub mod calibration;
pub mod clock;
pub mod effects;
#[cfg(feature = "lighting")]
pub mod lighting;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod modulators;
pub mod monitors;
/// 005-T1.2 — SVG path geometry extraction and arc-length parameterization.
pub mod path_geom;
pub mod project;
pub mod scene_pack;
/// P6.12.1 — Timecode sync decoders (MTC; LTC planned).
pub mod sync;
/// P6.5.1 — Transport state machine (session-only; not serialised).
pub mod transport;
/// Partial render module stub for the library crate. Exposes the
/// CPU-only, GPU-free sub-modules (`sdf`, `fx_presets`, `treatments`)
/// so that `project::audit` (v3-gated) can call registry lookups, and
/// `effects::Effect::Treatment` (PCleanup.1.3, unconditional) can name
/// `TreatmentPipeline` / `TreatmentInputs`, without depending on the
/// full `render/mod.rs` (which references `crate::windows` and
/// `crate::show_day`, both binary-only).
///
/// The binary crate (`main.rs`) declares `mod render;` normally,
/// loading the full `render/mod.rs`. Both module trees compile the
/// same underlying `.rs` files; the binary just gets more of them.
///
/// PCleanup.1.3 — gate removed. The render sub-modules compile without
/// `--features v3`; the previous gate was there because the only
/// library-side consumer (`project::audit`) is itself v3-gated. Now
/// that `effects::Effect::Treatment` (unconditional) names types from
/// `render::treatments`, the gate would force `Effect::Treatment` to
/// be v3-only, breaking schema deserialisation on non-v3 builds.
pub mod render {
    pub mod fx_compute;
    pub mod fx_fluid;
    pub mod fx_presets;
    pub mod sdf;
    // PCleanup.2.4 — Treatment-owned particle compute infrastructure.
    // Added here so that treatments.rs (which references TreatmentParticlePipeline)
    // can compile in the library crate (lib.rs) as well as the binary (main.rs).
    pub mod treatment_particles;
    pub mod treatments;
}
