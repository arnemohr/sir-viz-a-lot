//! T1.33 — once-per-machine onboarding toasts.
//!
//! Tracks persistent UI-education flags in
//! `~/Library/Application Support/rmap/ui_flags.json` so each notice
//! fires at most once per machine, not once per session.
//!
//! # Design
//!
//! - `read_flags` / `write_flags` use the production path; the `_at`
//!   variants accept an explicit path for unit-testing without touching
//!   `$HOME`.
//! - All disk operations are best-effort: read errors yield the default
//!   (all flags `false`); write errors are silently swallowed. A
//!   read-only home dir must not prevent the binary from running.
//! - The caller adds a **session latch** in `EditingState` that
//!   prevents a second push within the same session even when a disk
//!   write fails.

use std::path::PathBuf;

/// Persistent UI-education flags written to
/// `~/Library/Application Support/rmap/ui_flags.json`.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct UiFlags {
    /// Set `true` after the "Effects merged into the Layers tab as Look
    /// chain" onboarding toast has been shown. Prevents repeat shows on
    /// subsequent launches.
    #[serde(default)]
    pub look_chain_toast_seen: bool,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Resolve `~/Library/Application Support/rmap/ui_flags.json`.
/// Returns `None` only when `$HOME` is unset (should never happen in practice).
pub fn flags_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("rmap")
            .join("ui_flags.json"),
    )
}

// ---------------------------------------------------------------------------
// Public read / write API
// ---------------------------------------------------------------------------

/// Read `UiFlags` from the production path, or return the default (all
/// `false`) if the file is absent or cannot be parsed.
pub fn read_flags() -> UiFlags {
    match flags_path() {
        Some(p) => read_flags_at(&p),
        None => UiFlags::default(),
    }
}

/// Write `UiFlags` to the production path. Best-effort: errors are
/// swallowed so a read-only home dir cannot panic the binary.
pub fn write_flags(f: &UiFlags) {
    if let Some(p) = flags_path() {
        write_flags_at(&p, f);
    }
}

// ---------------------------------------------------------------------------
// Path-injected variants (test seam)
// ---------------------------------------------------------------------------

/// Read `UiFlags` from an explicit `path`. Returns the default when the
/// file is absent or unparseable.
pub fn read_flags_at(path: &std::path::Path) -> UiFlags {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return UiFlags::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

/// Write `UiFlags` to an explicit `path`. Creates parent directories if
/// needed. Errors are swallowed.
pub fn write_flags_at(path: &std::path::Path, f: &UiFlags) {
    let Ok(json) = serde_json::to_string(f) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, json);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// T1.33 AC: `UiFlags` round-trips through JSON. Uses a fixed temp
    /// path under `target/test-tmp/` rather than `tempfile` (which is
    /// not in dev-dependencies).
    #[test]
    fn ui_flags_round_trip_in_temp_dir() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join(format!(
                "ui_flags_round_trip_{}.json",
                std::process::id()
            ));

        // Start: flag absent — read should return default (false).
        // Remove any leftover from a prior run.
        let _ = std::fs::remove_file(&path);
        let loaded = read_flags_at(&path);
        assert!(!loaded.look_chain_toast_seen, "default must be false");

        // Write true, read back.
        let flags = UiFlags { look_chain_toast_seen: true };
        write_flags_at(&path, &flags);
        let reloaded = read_flags_at(&path);
        assert!(reloaded.look_chain_toast_seen, "persisted flag must be true");

        // Cleanup (best effort).
        let _ = std::fs::remove_file(&path);
    }

    /// `flags_path()` returns a path ending in `ui_flags.json` when
    /// `$HOME` is set (which it always is in the test environment).
    #[test]
    fn flags_path_ends_with_ui_flags_json() {
        if let Some(p) = flags_path() {
            assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("ui_flags.json"));
        }
        // If HOME is unset in a weird sandbox, the test just passes vacuously.
    }
}
