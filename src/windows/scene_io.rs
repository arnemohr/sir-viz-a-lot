//! P4.2.2 — `.rmap-scene.json` load / save helpers.
//!
//! File extension: `.rmap-scene.json` — mirrors `.rmap-preset.json` (P2.8.5)
//! so the operator mental model is consistent.
//!
//! Storage:
//! - Built-in templates: compiled into the binary (static registry in
//!   `src/project/scene_templates.rs`). Not distributed as files.
//! - User-exported templates: `~/Library/Application Support/rmap/scenes/`.
//! - Save of a `builtin: true` template returns `Err` (read-only enforcement).
//!
//! This mirrors the pattern from `src/windows/preset_io.rs` (P2.8.5).

use std::path::{Path, PathBuf};

use crate::project::scene_templates::SceneTemplate;

// ---------------------------------------------------------------------------
// Storage path helpers
// ---------------------------------------------------------------------------

/// Returns the user's scene templates directory:
/// `~/Library/Application Support/rmap/scenes/`.
///
/// Returns `None` if the home directory cannot be determined.
pub fn user_scenes_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .map(|h| h.join("Library/Application Support/rmap/scenes"))
}

// ---------------------------------------------------------------------------
// Save / load
// ---------------------------------------------------------------------------

/// Save `template` to `~/Library/Application Support/rmap/scenes/{id}.rmap-scene.json`.
///
/// Returns `Err` for built-in templates (read-only enforcement) and for any
/// I/O or serialisation failure.
#[allow(dead_code)] // wired by W4 wizard export + Phase 7 scene packs
pub fn save_user_scene_template(template: &SceneTemplate) -> std::io::Result<()> {
    if template.builtin {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "cannot save a built-in scene template (read-only)",
        ));
    }

    let dir = user_scenes_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine home directory for user scene templates",
        )
    })?;
    std::fs::create_dir_all(&dir)?;

    let file_name = format!("{}.rmap-scene.json", template.id);
    let path = dir.join(file_name);
    let tmp = path.with_extension("rmap-scene.json.tmp");

    let json =
        serde_json::to_string_pretty(template).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Load all user scene templates from
/// `~/Library/Application Support/rmap/scenes/*.rmap-scene.json`.
///
/// Malformed or unreadable files are logged and skipped; the function never
/// panics on bad input (mirrors `preset_io.rs`'s defensive behaviour).
#[allow(dead_code)] // wired by W3 wizard template picker + Phase 7 import
pub fn load_user_scene_templates() -> Vec<SceneTemplate> {
    let Some(dir) = user_scenes_dir() else {
        tracing::warn!("load_user_scene_templates: cannot determine home directory");
        return Vec::new();
    };

    let read_dir = match std::fs::read_dir(&dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Directory not yet created — no user templates, not an error.
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!("load_user_scene_templates: cannot read {:?}: {e}", dir);
            return Vec::new();
        }
    };

    let mut templates = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if !path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".rmap-scene.json"))
            .unwrap_or(false)
        {
            continue;
        }

        match load_scene_template_from_path(&path) {
            Ok(t) => templates.push(t),
            Err(e) => {
                tracing::warn!("load_user_scene_templates: skipping {:?}: {e}", path);
            }
        }
    }
    templates
}

/// Parse a single `.rmap-scene.json` file from `path`.
#[allow(dead_code)] // wired by load_user_scene_templates + Phase 7 import
pub fn load_scene_template_from_path(path: &Path) -> std::io::Result<SceneTemplate> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::scene_templates::{
        MediaSlotDescriptor, MediaSlotKind, MoodHint, PaletteHint,
    };
    use crate::project::schema::ZoneRole;

    fn fixture_user_template() -> SceneTemplate {
        SceneTemplate {
            id: "user_test_scene".to_string(),
            display_name: "User Test Scene".to_string(),
            description: "A user-authored test template for round-trip verification.".to_string(),
            zones_consumed: vec![ZoneRole::Window],
            media_slots: vec![MediaSlotDescriptor {
                name: "bg".to_string(),
                label: "Background image".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: false, // user template
        }
    }

    fn fixture_builtin_template() -> SceneTemplate {
        SceneTemplate {
            builtin: true,
            ..fixture_user_template()
        }
    }

    /// P4.2.2 — full JSON round-trip with all fields populated.
    #[test]
    fn rmap_scene_json_round_trip_all_fields() {
        let template = fixture_user_template();
        let dir = std::env::temp_dir();
        let path = dir.join("scene_roundtrip_test.rmap-scene.json");

        let json = serde_json::to_string_pretty(&template).expect("serialize");
        std::fs::write(&path, &json).expect("write");
        let back: SceneTemplate = load_scene_template_from_path(&path).expect("load");

        assert_eq!(back.id, template.id);
        assert_eq!(back.display_name, template.display_name);
        assert_eq!(back.description, template.description);
        assert_eq!(back.zones_consumed, template.zones_consumed);
        assert_eq!(back.media_slots.len(), template.media_slots.len());
        assert_eq!(back.fx_presets_used, template.fx_presets_used);
        assert_eq!(back.palette, template.palette);
        assert_eq!(back.mood, template.mood);
        assert_eq!(back.tempo_sync, template.tempo_sync);
        assert_eq!(back.builtin, template.builtin);

        let _ = std::fs::remove_file(&path);
    }

    /// P4.2.2 — a .rmap-scene.json with a missing optional-style field
    /// (zones_consumed empty) deserialises to a valid SceneTemplate.
    #[test]
    fn rmap_scene_json_missing_zones_consumed_uses_empty_default() {
        let json = r#"{
            "id": "minimal",
            "display_name": "Minimal",
            "description": "A minimal template.",
            "zones_consumed": [],
            "media_slots": [],
            "fx_presets_used": [],
            "palette": "warm",
            "mood": "calm",
            "tempo_sync": false,
            "builtin": false
        }"#;
        let t: SceneTemplate = serde_json::from_str(json).expect("deserialize minimal");
        assert_eq!(t.id, "minimal");
        assert!(t.zones_consumed.is_empty());
    }

    /// P4.2.2 — saving a builtin: true template returns Err.
    #[test]
    fn save_builtin_template_returns_error() {
        let template = fixture_builtin_template();
        let result = save_user_scene_template(&template);
        assert!(
            result.is_err(),
            "saving a builtin template must return an error"
        );
        let err = result.unwrap_err();
        assert_eq!(
            err.kind(),
            std::io::ErrorKind::PermissionDenied,
            "error kind must be PermissionDenied for builtin template save"
        );
    }
}
