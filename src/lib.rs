//! `rmap` library root — exposes crate modules for doctests and future
//! integration crates.
//!
//! The binary entry point lives in `src/main.rs`. This file exists so that
//! `cargo test --doc` can resolve crate items (e.g.
//! `rmap::project::command::ReverseStorage`) used in `compile_fail` doctests.

pub mod clock;
pub mod effects;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod modulators;
pub mod monitors;
pub mod project;
