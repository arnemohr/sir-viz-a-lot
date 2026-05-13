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

pub use schema::{CalibrationFile, CalibrationLoadError, CalibrationSurface, new_calibration_id};
