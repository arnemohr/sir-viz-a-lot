//! P7.7.1 — `CalibrationFile` schema + atomic save / load.
//!
//! These items are public library API; the binary crate includes this module
//! but has not yet wired every item to a call site.  The allow is module-
//! scoped so the binary's dead-code lint does not reject legit public API.
#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::schema::{BezierMesh, OutputTarget, WarpMesh};

// ---------------------------------------------------------------------------
// UUID generation helper
// ---------------------------------------------------------------------------

/// Generate a UUID v4-style string without pulling in the `uuid` crate.
///
/// Uses a mix of `std::time::SystemTime` and a call counter to produce a
/// string that is practically unique and stable across saves (the caller is
/// responsible for preserving the generated value across serialise/deserialise
/// cycles — the `calibration_id` field is serialised verbatim).
///
/// Format: `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx` (RFC 4122 UUID v4).
pub fn new_calibration_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 + d.as_secs() * 1_000_000_000)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);

    // Mix the bits using a simple non-cryptographic permutation.
    let a = nanos.wrapping_add(seq.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let b = a
        .rotate_left(17)
        .wrapping_add(seq.wrapping_mul(0x6c62_272e_07bb_0142));

    let hi = (a >> 32) as u32;
    let mid1 = ((a >> 16) & 0xffff) as u16;
    let mid2 = (((b >> 16) & 0x0fff) | 0x4000) as u16; // version 4
    let clock = ((b & 0x3fff) | 0x8000) as u16; // variant 1
    let lo = b & 0x0000_ffff_ffff_ffff;

    format!(
        "{hi:08x}-{mid1:04x}-{mid2:04x}-{clock:04x}-{lo:012x}",
        hi = hi,
        mid1 = mid1,
        mid2 = mid2,
        clock = clock,
        lo = lo
    )
}

// ---------------------------------------------------------------------------
// Warp type used in calibration files
// ---------------------------------------------------------------------------

/// P7.7.1 — Either a `BezierMesh` (v10+ calibration) or a legacy `WarpMesh`
/// (v1 calibration files written before Phase 7).
///
/// Calibration files have their own schema_version counter (starts at 1).
/// V1 calibration files may carry a `WarpMesh`; v2+ use `BezierMesh`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "warp_kind")]
pub enum WarpOrBezier {
    /// BezierMesh (Phase 7 canonical format).
    Bezier(BezierMesh),
    /// Legacy bilinear WarpMesh (pre-Phase 7 calibrations).
    Bilinear(WarpMesh),
}

impl WarpOrBezier {
    /// Return an identity `WarpOrBezier` (bilinear 1×1 identity).
    pub fn identity() -> Self {
        WarpOrBezier::Bilinear(WarpMesh::identity())
    }
}

// ---------------------------------------------------------------------------
// CalibrationSurface
// ---------------------------------------------------------------------------

/// P7.7.1 — Per-projector-surface calibration data.
///
/// `surface_slot_id` is a UUID string assigned when the surface is first
/// created; it is stable across saves.  The show file's `OutputTarget`
/// carries a matching `calibration_surface_slot_id` field so the runtime
/// can join them without path coupling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationSurface {
    /// Stable UUID string identifying this logical projector surface slot.
    /// Set once and preserved across all subsequent saves.
    pub surface_slot_id: String,
    /// Human-readable name for this surface (e.g. "Left projector").
    pub display_name: String,
    /// Physical display identity — same as `Project.output_targets[i]`.
    pub output_target: OutputTarget,
    /// Warp geometry for this surface (BezierMesh or legacy WarpMesh).
    pub warp: WarpOrBezier,
    /// Mask polygon in normalised [0..1] space.
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    /// Normalised mask feather (0..0.5 useful).
    #[serde(default = "default_feather")]
    pub mask_feather: f32,
    /// Per-projector 3×3 colour matrix (row-major, `out = matrix × in`).
    /// Identity default preserves show-file colour corrections.
    #[serde(default = "rgb_matrix_identity")]
    pub gamma_matrix: [[f32; 3]; 3],
    /// Per-projector brightness adjustment (0.0 = black, 1.0 = no change).
    #[serde(default = "default_one")]
    pub brightness: f32,
    /// Per-projector contrast adjustment (1.0 = no change).
    #[serde(default = "default_one")]
    pub contrast: f32,
}

fn default_feather() -> f32 {
    0.02
}

fn default_one() -> f32 {
    1.0
}

fn rgb_matrix_identity() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

impl CalibrationSurface {
    /// Create a new surface with a freshly-generated `surface_slot_id`.
    pub fn new(display_name: impl Into<String>, output_target: OutputTarget) -> Self {
        CalibrationSurface {
            surface_slot_id: new_calibration_id(),
            display_name: display_name.into(),
            output_target,
            warp: WarpOrBezier::identity(),
            mask_polygon: Vec::new(),
            mask_feather: 0.02,
            gamma_matrix: rgb_matrix_identity(),
            brightness: 1.0,
            contrast: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// CalibrationFile
// ---------------------------------------------------------------------------

/// P7.7.1 — Calibration file schema (`.rmap-calibration.json`).
///
/// Schema version starts at 1 and is independent of the project schema.
/// The `calibration_id` is stable across saves — do not regenerate on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationFile {
    /// Calibration file schema version (currently 1).
    pub schema_version: u32,
    /// Stable UUID identifying this calibration across saves.
    /// Generated once when the calibration is first created; preserved
    /// verbatim on every subsequent save.
    pub calibration_id: String,
    /// Human-readable venue name.
    pub venue_name: String,
    /// ISO 8601 timestamp of when this calibration was first created.
    pub created_at: String,
    /// Per-projector surface calibration slots.
    pub surfaces: Vec<CalibrationSurface>,
}

impl CalibrationFile {
    /// Current calibration file schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new calibration file for a venue.
    pub fn new(venue_name: impl Into<String>) -> Self {
        CalibrationFile {
            schema_version: Self::CURRENT_VERSION,
            calibration_id: new_calibration_id(),
            venue_name: venue_name.into(),
            created_at: iso8601_now(),
            surfaces: Vec::new(),
        }
    }

    /// Save this calibration file atomically to `path` (temp + rename).
    ///
    /// Returns `Err(CalibrationLoadError::Io)` if the temp file cannot be
    /// written or the rename fails.  Never panics.
    pub fn save(&self, path: &Path) -> Result<(), CalibrationLoadError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CalibrationLoadError::Parse(e.to_string()))?;

        // Atomic save: write to a temp file next to `path`, then rename.
        let parent = path.parent().ok_or_else(|| {
            CalibrationLoadError::Io(format!("path has no parent directory: {}", path.display()))
        })?;
        let tmp_path = parent.join(format!(".rmap-calibration-{}.tmp", std::process::id()));
        std::fs::write(&tmp_path, &json).map_err(|e| CalibrationLoadError::Io(e.to_string()))?;
        std::fs::rename(&tmp_path, path).map_err(|e| CalibrationLoadError::Io(e.to_string()))?;
        Ok(())
    }

    /// Load a calibration file from `path`.
    ///
    /// Returns `Err` on I/O failure or JSON parse error.  Never panics.
    pub fn load(path: &Path) -> Result<Self, CalibrationLoadError> {
        let bytes = std::fs::read(path).map_err(|e| CalibrationLoadError::Io(e.to_string()))?;
        let cal: CalibrationFile = serde_json::from_slice(&bytes)
            .map_err(|e| CalibrationLoadError::Parse(e.to_string()))?;
        Ok(cal)
    }
}

/// Return the current UTC time in a simple ISO 8601 string.
fn iso8601_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Simple conversion to HH:MM:SS UTC (no leap-second handling).
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Days since Unix epoch → Gregorian date (simple approximation).
    // Accurate for dates 1970-2100.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };

    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by [`CalibrationFile::load`] and [`CalibrationFile::save`].
#[derive(Debug, Error)]
pub enum CalibrationLoadError {
    /// File I/O error (read, write, or rename failure).
    #[error("calibration I/O error: {0}")]
    Io(String),
    /// JSON parse / serialization error.
    #[error("calibration parse error: {0}")]
    Parse(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::OutputTarget;

    /// P7.7.1 — `CalibrationFile` round-trips through JSON verbatim.
    /// `calibration_id` must survive serialise/deserialise unchanged.
    #[test]
    fn calibration_file_json_round_trip() {
        let mut cal = CalibrationFile::new("Test Venue");
        let original_id = cal.calibration_id.clone();
        let surface = CalibrationSurface::new("Main wall", OutputTarget::default());
        let surface_id = surface.surface_slot_id.clone();
        cal.surfaces.push(surface);

        let json = serde_json::to_string_pretty(&cal).expect("serialize");
        let restored: CalibrationFile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            restored.calibration_id, original_id,
            "calibration_id must survive round-trip"
        );
        assert_eq!(
            restored.surfaces.len(),
            1,
            "one surface must survive round-trip"
        );
        assert_eq!(
            restored.surfaces[0].surface_slot_id, surface_id,
            "surface_slot_id must survive round-trip"
        );
        assert_eq!(restored.venue_name, "Test Venue");
        assert_eq!(restored.schema_version, CalibrationFile::CURRENT_VERSION);
    }

    /// P7.7.1 — Atomic save: writes a temp file, renames it.
    /// The final path must contain valid JSON after save.
    #[test]
    fn calibration_file_atomic_save_and_load() {
        let tmp = std::env::temp_dir().join(format!(
            "rmap-test-calibration-{}.rmap-calibration.json",
            std::process::id()
        ));
        let cal = CalibrationFile::new("Atomic Save Test Venue");
        let id = cal.calibration_id.clone();

        cal.save(&tmp).expect("save");
        assert!(tmp.exists(), "saved file must exist");

        let loaded = CalibrationFile::load(&tmp).expect("load");
        assert_eq!(
            loaded.calibration_id, id,
            "calibration_id must survive atomic save + load"
        );
        assert_eq!(loaded.venue_name, "Atomic Save Test Venue");

        // Cleanup.
        let _ = std::fs::remove_file(&tmp);
    }

    /// P7.7.1 — `new_calibration_id` returns distinct values on successive calls.
    #[test]
    fn new_calibration_id_is_unique() {
        let a = new_calibration_id();
        let b = new_calibration_id();
        assert_ne!(a, b, "successive calibration IDs must differ");
        // Both must be 36 characters (UUID v4 string length).
        assert_eq!(a.len(), 36, "calibration ID must be 36 chars");
        assert_eq!(b.len(), 36, "calibration ID must be 36 chars");
    }

    /// P7.7.1 — Load returns `CalibrationLoadError::Io` for missing files.
    #[test]
    fn load_missing_file_returns_io_error() {
        let result = CalibrationFile::load(Path::new(
            "/nonexistent/path/calibration.rmap-calibration.json",
        ));
        assert!(
            matches!(result, Err(CalibrationLoadError::Io(_))),
            "missing file must return Io error"
        );
    }

    /// P7.7.1 — Load returns `CalibrationLoadError::Parse` for invalid JSON.
    #[test]
    fn load_invalid_json_returns_parse_error() {
        let tmp = std::env::temp_dir().join(format!(
            "rmap-test-bad-calibration-{}.rmap-calibration.json",
            std::process::id()
        ));
        std::fs::write(&tmp, b"not json").expect("write bad file");
        let result = CalibrationFile::load(&tmp);
        assert!(
            matches!(result, Err(CalibrationLoadError::Parse(_))),
            "invalid JSON must return Parse error"
        );
        let _ = std::fs::remove_file(&tmp);
    }
}
