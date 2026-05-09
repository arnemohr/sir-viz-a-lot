//! 003-T4.6 — debounced autosave to `~/Documents/rmap/_autosave/`.
//!
//! Each editor session writes to a fixed path derived from a per-session
//! token (`<pid>_<nanos>`). A 5-second quiet period is required before the
//! next write so rapid-fire mutations (e.g. dragging a warp corner) don't
//! thrash disk. The token never changes for the session lifetime, so old
//! files from prior sessions remain in `_autosave/` as recovery candidates
//! until the operator explicitly opens a named project.
//!
//! Recovery scanning (`scan_autosave_recovery` in `recents.rs`) picks up
//! the most-recently-modified autosave file and surfaces it in the launcher
//! as "Last session (recovery)".

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::projects_dir::projects_dir;

/// Autosave quiet period: 5 seconds after the last mutation before writing.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(5);

/// Resolve the autosave directory: `~/Documents/rmap/_autosave/`.
///
/// Falls back to `/tmp/rmap_autosave` if `HOME` is unset (e.g. in test
/// environments that strip the environment). The fallback path is created
/// on the same `maybe_autosave` call that first needs it; creation failure
/// is non-fatal (the save is simply skipped with a `tracing::warn`).
pub fn autosave_dir() -> PathBuf {
    projects_dir()
        .map(|p| p.join("_autosave"))
        .unwrap_or_else(|| std::env::temp_dir().join("rmap_autosave"))
}

/// Full path for a session's autosave file.
pub fn autosave_path(session_token: &str) -> PathBuf {
    autosave_dir().join(format!("{session_token}.rmap.json"))
}

/// Pure predicate: should an autosave write happen right now?
///
/// Returns `true` when:
/// - `dirty` is `true` (there are unsaved mutations), AND
/// - `last_req` is `None` (no autosave has been attempted yet this session),
///   OR the last attempt was more than `AUTOSAVE_DEBOUNCE` ago.
///
/// Parameterised over `now` so it's unit-testable without mocking the clock.
pub fn should_autosave(now: Instant, last_req: Option<Instant>, dirty: bool) -> bool {
    if !dirty {
        return false;
    }
    match last_req {
        None => true,
        Some(t) => now.saturating_duration_since(t) >= AUTOSAVE_DEBOUNCE,
    }
}

/// Attempt an autosave for the current session if the debounce window has
/// elapsed and the project is dirty. Returns `true` if a write was attempted
/// (regardless of whether it succeeded), so the caller can update
/// `last_autosave_request`. On success the dirty flag is cleared.
///
/// Write errors are logged via `tracing::warn` and are otherwise non-fatal —
/// the operator can still save manually via Save / Save as….
pub fn maybe_autosave(
    project: &crate::project::schema::Project,
    session_token: &str,
    dirty: &mut bool,
    last_autosave_request: &mut Option<Instant>,
) -> bool {
    let now = Instant::now();
    if !should_autosave(now, *last_autosave_request, *dirty) {
        return false;
    }

    // Record the attempt time before the write so a slow/failing write
    // doesn't immediately retry on every subsequent frame.
    *last_autosave_request = Some(now);

    let path = autosave_path(session_token);

    // Ensure the autosave directory exists. If `bootstrap()` was called at
    // startup this is a no-op; if HOME was absent at startup the fallback
    // `/tmp/rmap_autosave` is created here.
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(
                path = %parent.display(),
                ?err,
                "autosave: couldn't create autosave directory; skipping write",
            );
            return true; // attempt was made, even if it failed
        }
    }

    match project.save(&path) {
        Ok(()) => {
            *dirty = false;
            tracing::debug!(
                path = %path.display(),
                "autosave: written",
            );
        }
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                ?err,
                "autosave: write failed",
            );
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T4.6 acceptance: `autosave_path` embeds the session token.
    #[test]
    fn autosave_path_uses_token() {
        let token = "12345_9876543210";
        let path = autosave_path(token);
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assert!(
            name.contains(token),
            "autosave file name should contain the session token; got {name}"
        );
        assert!(
            name.ends_with(".rmap.json"),
            "autosave file should have .rmap.json extension; got {name}"
        );
    }

    /// T4.6 acceptance: pure debounce logic — no disk I/O needed.
    #[test]
    fn should_autosave_debounce_logic() {
        let t0 = Instant::now();

        // Not dirty → never autosave.
        assert!(!should_autosave(t0, None, false));
        assert!(!should_autosave(t0, Some(t0), false));

        // Dirty, no prior request → autosave immediately.
        assert!(should_autosave(t0, None, true));

        // Dirty, recent request → debounce suppresses.
        let recent = t0; // same instant — well inside debounce window
        assert!(!should_autosave(t0, Some(recent), true));

        // Dirty, old request (>= 5 s ago) → autosave allowed.
        let old = t0
            .checked_sub(AUTOSAVE_DEBOUNCE)
            .expect("subtract debounce");
        assert!(should_autosave(t0, Some(old), true));
    }
}
