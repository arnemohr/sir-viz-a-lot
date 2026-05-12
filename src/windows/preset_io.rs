//! P2.8.4–P2.8.5 — `.rmap-preset.json` schema, read, and write helpers.
//!
//! A `.rmap-preset.json` file is the single-file preset transport format.
//! Schema: `{ "preset_id": String, "params": HashMap<String, f32>, "name": String,
//! "author": Option<String> }`. No media, no warp, no mask — only the preset
//! identifier and its param overrides.

use std::collections::HashMap;
use std::path::Path;

/// Transport format for a single FX preset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RmapPresetJson {
    /// Registered `FxPresetEntry::preset_id` (e.g. `"mask_edge_ripple_wash"`).
    pub preset_id: String,
    /// Per-preset parameter overrides. Missing keys fall back to descriptor
    /// defaults at apply time; extra keys are silently ignored.
    pub params: HashMap<String, f32>,
    /// Human-readable name (shown in the browser).
    pub name: String,
    /// Optional author attribution.
    pub author: Option<String>,
}

/// Write `preset` to `path` as pretty-printed JSON.
///
/// Uses an atomic temp-file + rename pattern to avoid partial writes.
pub fn write_preset(path: &Path, preset: &RmapPresetJson) -> std::io::Result<()> {
    let json =
        serde_json::to_string_pretty(preset).map_err(|e| std::io::Error::other(e.to_string()))?;
    // Atomic: write to a sibling temp file then rename.
    let tmp = path.with_extension("rmap-preset.json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read and parse a `.rmap-preset.json` file from `path`.
pub fn read_preset(path: &Path) -> std::io::Result<RmapPresetJson> {
    let s = std::fs::read_to_string(path)?;
    serde_json::from_str(&s)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// P2.8.4 — write + read back produces identical fields.
    #[test]
    fn rmap_preset_json_round_trip() {
        let mut params = HashMap::new();
        params.insert("speed".to_string(), 2.5_f32);
        params.insert("falloff".to_string(), 0.1_f32);
        let preset = RmapPresetJson {
            preset_id: "mask_edge_ripple_wash".to_string(),
            params,
            name: "My Ripple".to_string(),
            author: Some("Alice".to_string()),
        };
        let dir = std::env::temp_dir();
        let path = dir.join("test_preset.rmap-preset.json");
        write_preset(&path, &preset).expect("write_preset should succeed");
        let back = read_preset(&path).expect("read_preset should succeed");
        assert_eq!(back.preset_id, preset.preset_id);
        assert_eq!(back.name, preset.name);
        assert_eq!(back.author, preset.author);
        assert_eq!(back.params.get("speed"), preset.params.get("speed"));
        assert_eq!(back.params.get("falloff"), preset.params.get("falloff"));
        let _ = std::fs::remove_file(&path);
    }

    /// P2.8.5 — write to a temp dir, read back, fields match.
    #[test]
    fn rmap_preset_json_export_import_round_trip() {
        let mut params = HashMap::new();
        params.insert("particle_count".to_string(), 128.0_f32);
        let preset = RmapPresetJson {
            preset_id: "mask_constrained_drift".to_string(),
            params: params.clone(),
            name: "Export test".to_string(),
            author: None,
        };
        let dir = std::env::temp_dir();
        let path = dir.join("export_test.rmap-preset.json");
        write_preset(&path, &preset).expect("write_preset");
        let back = read_preset(&path).expect("read_preset");
        assert_eq!(back.preset_id, "mask_constrained_drift");
        assert_eq!(back.name, "Export test");
        assert_eq!(back.author, None);
        assert_eq!(back.params.get("particle_count"), Some(&128.0_f32));
        let _ = std::fs::remove_file(&path);
    }

    /// P2.8.5 — `fx_is_registered` returns false for an unknown preset_id.
    #[test]
    fn import_unknown_preset_id_returns_error() {
        assert!(
            !crate::render::fx_presets::fx_is_registered("definitely_fake"),
            "definitely_fake should not be registered"
        );
    }
}
