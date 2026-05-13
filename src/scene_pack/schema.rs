//! P7.10.1 — `ScenePackManifest`, `ScenePackTemplate`, export, and import.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::calibration::schema::new_calibration_id;
use crate::project::schema::LayerConfig;

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// A single scene template inside a scene pack.
///
/// Wraps a `LayerConfig` (the full layer including warp, effects, kind)
/// and records the normalised zip paths to any referenced assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePackTemplate {
    /// Human-readable label for this template in the Preset Browser.
    pub name: String,
    /// The layer configuration (warp, effects, mask, etc.).
    pub layer: LayerConfig,
    /// Zip-relative paths to referenced assets (images, SVGs, .rmap-preset.json).
    /// Paths use forward-slash separators; assets live under `assets/<template_idx>/`.
    #[serde(default)]
    pub asset_paths: Vec<String>,
}

/// Manifest stored as `manifest.json` at the root of the zip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePackManifest {
    /// Manifest schema version (currently 1; independent of project schema).
    pub schema_version: u32,
    /// Stable UUID identifying this pack across re-exports.
    pub pack_id: String,
    /// Human-readable pack name.
    pub name: String,
    /// Author name (optional).
    #[serde(default)]
    pub author: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// List of scene templates included in this pack.
    pub templates: Vec<ScenePackTemplate>,
}

impl ScenePackManifest {
    /// Current manifest schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new manifest with a freshly-generated `pack_id`.
    pub fn new(name: impl Into<String>, author: impl Into<String>) -> Self {
        ScenePackManifest {
            schema_version: Self::CURRENT_VERSION,
            pack_id: new_calibration_id(),
            name: name.into(),
            author: author.into(),
            created_at: iso8601_now(),
            templates: Vec::new(),
        }
    }

    /// Export this manifest + the referenced assets from `asset_root` to
    /// `path` as a `.rmap-scene-pack.zip`.
    ///
    /// `asset_root` is the directory from which asset paths are resolved.
    /// Assets are stored under `assets/<template_idx>/<filename>` in the zip.
    ///
    /// Writes atomically: a temp file is created first, then renamed.
    pub fn export(&self, path: &Path, asset_root: &Path) -> Result<(), ScenePackError> {
        let parent = path
            .parent()
            .ok_or_else(|| ScenePackError::Io(format!("path has no parent: {}", path.display())))?;
        let tmp_path = parent.join(format!(".scene-pack-{}.tmp", std::process::id()));

        let tmp_file =
            std::fs::File::create(&tmp_path).map_err(|e| ScenePackError::Io(e.to_string()))?;
        let mut zip = zip::ZipWriter::new(tmp_file);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Write manifest.json.
        let manifest_json =
            serde_json::to_string_pretty(self).map_err(|e| ScenePackError::Parse(e.to_string()))?;
        zip.start_file("manifest.json", options)
            .map_err(|e| ScenePackError::Zip(e.to_string()))?;
        zip.write_all(manifest_json.as_bytes())
            .map_err(|e| ScenePackError::Io(e.to_string()))?;

        // Write assets.
        for (tmpl_idx, tmpl) in self.templates.iter().enumerate() {
            for zip_path in &tmpl.asset_paths {
                // Resolve to a filesystem path.
                let local_path = zip_path
                    .split('/')
                    .fold(asset_root.to_path_buf(), |acc, part| acc.join(part));
                if !local_path.exists() {
                    // Skip missing assets — don't fail the export.
                    tracing::warn!(
                        template = tmpl.name,
                        asset = zip_path,
                        "scene pack export: asset not found, skipping"
                    );
                    continue;
                }
                let data =
                    std::fs::read(&local_path).map_err(|e| ScenePackError::Io(e.to_string()))?;
                let asset_entry = format!("assets/{tmpl_idx}/{zip_path}");
                zip.start_file(&asset_entry, options)
                    .map_err(|e| ScenePackError::Zip(e.to_string()))?;
                zip.write_all(&data)
                    .map_err(|e| ScenePackError::Io(e.to_string()))?;
            }
        }

        zip.finish()
            .map_err(|e| ScenePackError::Zip(e.to_string()))?;

        std::fs::rename(&tmp_path, path).map_err(|e| ScenePackError::Io(e.to_string()))?;
        Ok(())
    }

    /// Import a `.rmap-scene-pack.zip` from `path`.
    ///
    /// Extracts to `dest_dir/<pack_id>/`.  Returns the loaded manifest.
    /// If `dest_dir/<pack_id>/` already exists, it is replaced.
    pub fn import(
        path: &Path,
        dest_dir: &Path,
    ) -> Result<(ScenePackManifest, PathBuf), ScenePackError> {
        let file = std::fs::File::open(path).map_err(|e| ScenePackError::Io(e.to_string()))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| ScenePackError::Zip(e.to_string()))?;

        // Load manifest first.
        let manifest: ScenePackManifest = {
            let mut mf = zip
                .by_name("manifest.json")
                .map_err(|_| ScenePackError::MissingManifest)?;
            let mut buf = String::new();
            mf.read_to_string(&mut buf)
                .map_err(|e| ScenePackError::Io(e.to_string()))?;
            serde_json::from_str(&buf).map_err(|e| ScenePackError::Parse(e.to_string()))?
        };

        let pack_dir = dest_dir.join(&manifest.pack_id);

        // Remove existing pack dir if present (same-ID re-import = replace).
        if pack_dir.exists() {
            std::fs::remove_dir_all(&pack_dir).map_err(|e| ScenePackError::Io(e.to_string()))?;
        }
        std::fs::create_dir_all(&pack_dir).map_err(|e| ScenePackError::Io(e.to_string()))?;

        // Extract all entries.
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| ScenePackError::Zip(e.to_string()))?;
            let entry_name = entry.name().to_string();
            let dest_path = pack_dir.join(entry_name.replace('/', std::path::MAIN_SEPARATOR_STR));

            if entry.is_dir() {
                std::fs::create_dir_all(&dest_path)
                    .map_err(|e| ScenePackError::Io(e.to_string()))?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| ScenePackError::Io(e.to_string()))?;
                }
                let mut data = Vec::new();
                entry
                    .read_to_end(&mut data)
                    .map_err(|e| ScenePackError::Io(e.to_string()))?;
                std::fs::write(&dest_path, &data).map_err(|e| ScenePackError::Io(e.to_string()))?;
            }
        }

        Ok((manifest, pack_dir))
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by scene pack export / import.
#[derive(Debug, Error)]
pub enum ScenePackError {
    /// I/O error (file read, write, create, or rename failure).
    #[error("scene pack I/O error: {0}")]
    Io(String),
    /// JSON parse or serialization error.
    #[error("scene pack parse error: {0}")]
    Parse(String),
    /// Zip archive error (corrupt archive, missing entry, etc.).
    #[error("scene pack zip error: {0}")]
    Zip(String),
    /// The zip does not contain `manifest.json` at its root.
    #[error("scene pack is missing manifest.json")]
    MissingManifest,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn iso8601_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// P7.10.1 — `ScenePackManifest` round-trips through JSON.
    #[test]
    fn manifest_json_round_trip() {
        let m = ScenePackManifest::new("Test Pack", "Test Author");
        let original_id = m.pack_id.clone();
        let json = serde_json::to_string_pretty(&m).expect("serialize");
        let restored: ScenePackManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            restored.pack_id, original_id,
            "pack_id must survive round-trip"
        );
        assert_eq!(restored.name, "Test Pack");
        assert_eq!(restored.schema_version, ScenePackManifest::CURRENT_VERSION);
    }

    /// P7.10.1 — Export + Import round-trip: manifest survives zip.
    #[test]
    fn export_import_round_trip_manifest() {
        let tmp = std::env::temp_dir().join(format!(
            "rmap-test-scene-pack-{}.rmap-scene-pack.zip",
            std::process::id()
        ));
        let dest =
            std::env::temp_dir().join(format!("rmap-test-scene-pack-dest-{}", std::process::id()));
        std::fs::create_dir_all(&dest).expect("create dest");

        let m = ScenePackManifest::new("Round-trip Pack", "CI");
        let pack_id = m.pack_id.clone();

        // Export (no assets).
        m.export(&tmp, std::env::temp_dir().as_path())
            .expect("export");
        assert!(tmp.exists(), "zip must exist after export");

        // Import.
        let (restored, pack_dir) = ScenePackManifest::import(&tmp, &dest).expect("import");
        assert_eq!(
            restored.pack_id, pack_id,
            "pack_id must survive export+import"
        );
        assert_eq!(restored.name, "Round-trip Pack");
        assert!(pack_dir.exists(), "pack directory must exist after import");
        assert!(
            pack_dir.join("manifest.json").exists(),
            "manifest.json must be extracted"
        );

        // Re-import of same pack_id replaces the directory (no duplicate).
        let (restored2, _) = ScenePackManifest::import(&tmp, &dest).expect("re-import");
        assert_eq!(
            restored2.pack_id, pack_id,
            "re-import must produce same pack_id"
        );

        // Cleanup.
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_dir_all(&pack_dir);
    }

    /// P7.10.1 — Import from a missing file returns Io error.
    #[test]
    fn import_missing_file_returns_io_error() {
        let result = ScenePackManifest::import(
            Path::new("/nonexistent/scene.rmap-scene-pack.zip"),
            std::env::temp_dir().as_path(),
        );
        assert!(
            matches!(result, Err(ScenePackError::Io(_))),
            "missing file must return Io error"
        );
    }
}
