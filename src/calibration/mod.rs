//! P7.7.1 — Calibration file schema, save, and load.
//!
//! A `.rmap-calibration.json` file stores warp + mask + gamma + display
//! identity per venue, separate from any show file.  Binding is by
//! `surface_slot_id` UUID (matched against `OutputTarget.uuid` in the show
//! file).  Mismatch = audit warning + identity fallback; never hard-fail.
//!
//! The file is saved atomically (temp + rename) following the same pattern
//! as `Project::save`.

pub mod schema;

// These items are public library API (used by the lib crate and future
// operator UI code).  The binary crate declares this module but has not
// yet wired every item to a call site; suppress the false-positive lints.
#[allow(unused_imports)]
pub use schema::{CalibrationFile, CalibrationLoadError, CalibrationSurface, new_calibration_id};
