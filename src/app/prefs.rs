//! Operator-level preferences persisted across sessions.
//!
//! 003-T2.18 — first iteration ships two fields:
//!
//! - `last_used_projector_uuid` (T-003-T2.20): stable id of the
//!   projector the operator last sent to, so the launcher's projector
//!   dropdown can preselect it on the next cold start.
//! - `first_launch_completed` (T-003-T2.4): one-shot flag that
//!   suppresses the "Recommended" badge on the demo button after the
//!   operator has launched at least once.
//!
//! On macOS the file lives at `~/Library/Preferences/rmap.toml`.
//! Everywhere else we honour `XDG_CONFIG_HOME` (or `~/.config`) per
//! the Linux base-directory spec — Phase 2's first-launch experience
//! is mac-only, but the prefs schema is portable so the same file
//! works once the rest of the app catches up.
//!
//! Three properties matter for the load path:
//!
//! 1. **No-prefs-file launches succeed silently** — first-cold-start
//!    is the most common case; surfacing a "missing file" warning
//!    in the operator's log every launch would be noise.
//! 2. **A corrupt file falls back to defaults**, not a crash.
//!    The user can have hand-edited the file or a previous version
//!    can have written something we no longer parse.
//! 3. **Unknown keys are ignored**. Future schema additions must
//!    not break older binaries reading newer files (e.g. an operator
//!    who downgrades for show-day reliability).
//!
//! Save uses a tempfile + rename so partial writes never replace a
//! good file with a corrupt one.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Operator-level preferences, persisted at the per-user OS-native
/// location. See module docs for the file path.
///
/// `#[serde(default)]` on the struct delegates each missing field
/// to the field type's `Default`; combined with serde-toml's default
/// behaviour of ignoring unknown keys, this gives forward-compatible
/// schema evolution: older binaries silently drop new fields the
/// schema knows about, and never reject the file outright.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPrefs {
    /// 003-T2.20 — stable identifier of the projector the operator
    /// last sent to. macOS ships the `CGDirectDisplayID` as a decimal
    /// string (see `crate::monitors::stable_id`). Other platforms
    /// leave this `None` until a stable id source lands; the launcher
    /// dropdown still works, it just can't preselect last-used.
    pub last_used_projector_uuid: Option<String>,
    /// 003-T2.4 — flipped to `true` the first time the operator clicks
    /// any launcher start button. The "Recommended" badge on the demo
    /// button is suppressed once this is set.
    pub first_launch_completed: bool,
}

impl UserPrefs {
    /// Load preferences from the canonical OS-native path. Returns
    /// `Default` on:
    ///
    /// - missing file (silent — debug-log only)
    /// - permission / I/O error (warn-log)
    /// - parse error (warn-log)
    /// - unresolvable path (e.g. `HOME` env var unset; warn-log)
    ///
    /// All four cases produce a `UserPrefs::default()` rather than a
    /// panic so the launcher always opens regardless of the prefs
    /// state.
    pub fn load() -> Self {
        let Some(path) = canonical_path() else {
            tracing::warn!("UserPrefs::load: cannot resolve preferences path; using defaults",);
            return Self::default();
        };
        Self::load_from_path(&path)
    }

    /// Path-driven `load` for unit tests. Public-in-crate so the load
    /// logic itself is exercised against tempfile fixtures rather than
    /// the user's real `~/Library/Preferences/rmap.toml`.
    pub(crate) fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<UserPrefs>(&text) {
                Ok(prefs) => prefs,
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "UserPrefs::load: parse failure; falling back to defaults",
                    );
                    Self::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = %path.display(), "UserPrefs::load: no prefs file yet");
                Self::default()
            }
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    ?err,
                    "UserPrefs::load: read error; falling back to defaults",
                );
                Self::default()
            }
        }
    }

    /// Save preferences to the canonical path. Creates parent
    /// directories as needed; writes atomically via tempfile + rename.
    /// I/O errors propagate so the caller can surface a toast (T-003-T2.19
    /// owns the user-facing failure message for filesystem trouble).
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = canonical_path() else {
            tracing::warn!("UserPrefs::save: cannot resolve preferences path; not saving");
            return Ok(());
        };
        self.save_to_path(&path)
    }

    /// Path-driven `save` for unit tests. Same atomicity guarantees
    /// as `save`.
    pub(crate) fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("serialize: {e}"))
        })?;
        let tmp = path.with_extension("toml.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(serialized.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// macOS: `~/Library/Preferences/rmap.toml`. Other platforms:
/// `$XDG_CONFIG_HOME/rmap/rmap.toml` (or `~/.config/rmap/rmap.toml`).
/// Returns `None` if neither `HOME` nor `XDG_CONFIG_HOME` is set —
/// extremely rare; an unset `HOME` is a system-misconfiguration we
/// log and degrade to in-memory-only prefs for the session.
fn canonical_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Preferences/rmap.toml"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("rmap").join("rmap.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rmap_t2_18_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            name,
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir.join("rmap.toml")
    }

    /// 003-T2.18 acceptance criterion 1 (defaults on missing file)
    /// + criterion 3 (graceful failure modes).
    #[test]
    fn load_returns_default_when_file_missing() {
        let path = temp_path("missing").with_file_name("nope.toml");
        let prefs = UserPrefs::load_from_path(&path);
        assert_eq!(prefs, UserPrefs::default());
    }

    /// 003-T2.18 acceptance criterion 2: a corrupt prefs file must
    /// not crash rmap; load returns the default.
    #[test]
    fn load_returns_default_on_corrupt_file() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"this isn't valid toml = = =\nmalformed").expect("write fixture");
        let prefs = UserPrefs::load_from_path(&path);
        assert_eq!(prefs, UserPrefs::default());
        let _ = std::fs::remove_file(&path);
    }

    /// 003-T2.18 acceptance criterion 3: unknown keys must be ignored
    /// so a future-version file still loads on an older binary.
    #[test]
    fn load_ignores_unknown_keys() {
        let path = temp_path("forward-compat");
        std::fs::write(
            &path,
            "first_launch_completed = true\nfuture_field = \"hello\"\n",
        )
        .expect("write fixture");
        let prefs = UserPrefs::load_from_path(&path);
        assert!(
            prefs.first_launch_completed,
            "known key should round-trip even when unknown keys are present"
        );
        assert!(
            prefs.last_used_projector_uuid.is_none(),
            "missing key should fall back to type Default"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// 003-T2.18 acceptance criterion 1: save then load round-trips
    /// the values intact.
    #[test]
    fn save_then_load_round_trips() {
        let path = temp_path("round-trip");
        let original = UserPrefs {
            last_used_projector_uuid: Some("12345".into()),
            first_launch_completed: true,
        };
        original.save_to_path(&path).expect("save");
        let loaded = UserPrefs::load_from_path(&path);
        assert_eq!(loaded, original);
        let _ = std::fs::remove_file(&path);
    }
}
