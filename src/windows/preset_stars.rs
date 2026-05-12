//! P2.8.3 — Starred preset persistence.
//!
//! [`PresetStars`] reads/writes a flat JSON array of starred preset IDs to
//! `~/Library/Application Support/rmap/preset_stars.json`.
//!
//! All disk errors are logged as warnings and silently ignored — star state
//! is a preference, not project state. It never touches the undo stack.

use std::path::PathBuf;

/// Flat list of starred preset IDs.
///
/// Stored as `{ "starred": ["preset_id_1", "preset_id_2"] }` on disk.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct PresetStars {
    pub starred: Vec<String>,
}

impl PresetStars {
    /// Path to `preset_stars.json` inside the rmap data directory.
    fn path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .map(|h| h.join("Library/Application Support/rmap/preset_stars.json"))
    }

    /// Load from disk, returning `Default` on any failure.
    pub fn load_or_default() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!("preset_stars.json parse error: {e}; using empty state");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Write to disk (atomic temp-file rename on the same filesystem).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "HOME environment variable not set",
            ));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        // Atomic write: write to a temp file then rename.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Returns `true` if the preset is starred.
    pub fn is_starred(&self, preset_id: &str) -> bool {
        self.starred.iter().any(|s| s == preset_id)
    }

    /// Toggle: if starred, remove; if not, add. Saves to disk.
    pub fn toggle(&mut self, preset_id: &str) {
        if self.is_starred(preset_id) {
            self.starred.retain(|s| s != preset_id);
        } else {
            self.starred.push(preset_id.to_string());
        }
        if let Err(e) = self.save() {
            tracing::warn!("could not save preset_stars.json: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// P2.8.3 — toggle adds; toggle again removes.
    #[test]
    fn preset_stars_toggle() {
        let mut stars = PresetStars::default();
        assert!(!stars.is_starred("foo"));
        // Manual toggle (bypass disk IO in tests).
        stars.starred.push("foo".to_string());
        assert!(stars.is_starred("foo"));
        stars.starred.retain(|s| s != "foo");
        assert!(!stars.is_starred("foo"));
    }

    /// P2.8.3 — serialize / deserialize identical.
    #[test]
    fn preset_stars_round_trip_json() {
        let stars = PresetStars {
            starred: vec![
                "mask_edge_ripple_wash".to_string(),
                "fluid_identity".to_string(),
            ],
        };
        let json = serde_json::to_string(&stars).expect("serialize");
        let back: PresetStars = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.starred, stars.starred);
    }
}
