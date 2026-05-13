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
pub mod project;
pub mod scene_pack;
/// P6.12.1 — Timecode sync decoders (MTC; LTC planned).
pub mod sync;
/// P6.5.1 — Transport state machine (session-only; not serialised).
pub mod transport;
/// Partial render module stub for the library crate. Exposes the
/// CPU-only, GPU-free sub-modules (`sdf`, `fx_presets`, `treatments`)
/// so that `project::audit` (v3-gated) can call registry lookups
/// without depending on the full `render/mod.rs` (which references
/// `crate::windows` and `crate::show_day`, both binary-only).
///
/// The binary crate (`main.rs`) declares `mod render;` normally,
/// loading the full `render/mod.rs`. Both module trees compile the
/// same underlying `.rs` files; the binary just gets more of them.
#[cfg(feature = "v3")]
pub mod render {
    pub mod fx_compute;
    pub mod fx_fluid;
    pub mod fx_presets;
    pub mod sdf;
    pub mod treatments;
}
