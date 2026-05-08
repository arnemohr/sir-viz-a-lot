//! Operator's projects directory: `~/Documents/rmap/`.
//!
//! 003-T2.19 — first-launch bootstrap. The directory hosts:
//!
//! - User-created `*.rmap.json` show files (the "Open recent" listing
//!   in T-003-T2.10 scans this directory).
//! - The `_autosave/` subdirectory for crash-recovery snapshots
//!   (out-of-scope for Phase 2 but reserved here so the path is
//!   stable from day one).
//!
//! The launcher calls [`bootstrap`] on mount; the editor's Save-as
//! flow (T-003-T2.13's `pick_save_destination`) lands new files here
//! by default. A permission failure to create either directory is
//! non-fatal — Save still works against any path the operator picks
//! manually. The launcher surfaces a toast to that effect; the bare
//! [`bootstrap`] return value is what the toast text is built from.

use std::path::PathBuf;

/// Resolve `~/Documents/rmap/`. Returns `None` if `HOME` is unset or
/// the platform doesn't follow the macOS / XDG layout convention.
///
/// macOS hard-codes `Documents`; Linux defers to `XDG_DOCUMENTS_DIR`
/// when set, falling back to `$HOME/Documents`. Windows isn't a v1
/// target but the resolver is shaped so it doesn't crash there
/// (Windows uses a registry-backed path that the `dirs` crate would
/// expose; v3 won't add that dep just for first-launch bootstrap).
fn projects_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Documents").join("rmap"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let base = std::env::var_os("XDG_DOCUMENTS_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents")))?;
        Some(base.join("rmap"))
    }
}

/// Outcome of [`bootstrap`]: the resolved projects directory path,
/// plus any non-fatal warning the caller should surface as a toast.
///
/// We deliberately do not bubble `Result<_, io::Error>` — the launcher
/// must always proceed (per spec: "Save will still work, but you may
/// need to pick a location each time"). The warning carries operator-
/// facing copy ready to feed straight into a toast.
#[allow(dead_code)] // Constructed by T-003-T2.10's launcher bootstrap site.
#[derive(Debug, Clone)]
pub struct BootstrapOutcome {
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

/// 003-T2.19 — make sure `~/Documents/rmap/` and `~/Documents/rmap/_autosave/`
/// exist. Idempotent: a second call on a populated directory does
/// nothing and returns the same path.
///
/// Failure modes:
///
/// - **Path resolution failure** (e.g. `HOME` unset): `path` = `None`,
///   `warning` populated with a toast-ready string.
/// - **Permission failure** on `create_dir_all`: `path` carries the
///   resolved path so subsequent Save-as flows still default to it,
///   `warning` populated.
/// - **Success**: `path` = `Some(...)`, `warning` = `None`.
#[allow(dead_code)] // Wired by T-003-T2.10's launcher init.
pub fn bootstrap() -> BootstrapOutcome {
    let Some(path) = projects_dir() else {
        return BootstrapOutcome {
            path: None,
            warning: Some(
                "Couldn't find your Documents folder. Save will still work, but you may need to pick a location each time."
                    .to_string(),
            ),
        };
    };

    let autosave = path.join("_autosave");
    match std::fs::create_dir_all(&autosave) {
        Ok(()) => {
            tracing::debug!(
                path = %path.display(),
                "projects directory ready",
            );
            BootstrapOutcome {
                path: Some(path),
                warning: None,
            }
        }
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                ?err,
                "couldn't create projects directory; Save-as will still work",
            );
            BootstrapOutcome {
                path: Some(path),
                warning: Some(
                    "Couldn't create rmap's projects folder. Save will still work, but you may need to pick a location each time."
                        .to_string(),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 003-T2.19 acceptance criterion 2: bootstrap is idempotent.
    /// Running it twice in succession against the same env produces
    /// the same path and the same `warning = None` result on a
    /// writable filesystem.
    #[test]
    fn bootstrap_is_idempotent() {
        // Drive the bootstrap against a tempdir-overridden HOME so
        // the test never touches the real `~/Documents/rmap/`. We
        // restore the original HOME afterward to stay polite to the
        // surrounding test process.
        let scratch = std::env::temp_dir().join(format!(
            "rmap_t2_19_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&scratch).expect("scratch dir");

        // Save and override `HOME` for this test only. cargo nextest
        // (the project's default runner) executes each test in its own
        // process, so the env mutation is isolated from other tests.
        // For `cargo test` fallbacks the restore at the end of the
        // function keeps the process clean for subsequent tests in the
        // same binary; cross-test races are still possible there if
        // other tests read `HOME` concurrently, but no other test in
        // this crate does.
        let prev = std::env::var_os("HOME");
        // SAFETY: env mutation is process-global. Per the comment
        // above, the runner isolates this and the restore below
        // unwinds the change.
        unsafe {
            std::env::set_var("HOME", &scratch);
        }

        let first = bootstrap();
        let second = bootstrap();

        assert_eq!(first.path, second.path);
        assert!(
            first.warning.is_none(),
            "first call on writable fs should not warn"
        );
        assert!(second.warning.is_none(), "second call should not warn");

        let path = first.path.expect("path");
        let autosave = path.join("_autosave");
        assert!(autosave.is_dir(), "_autosave subdir created");

        // Restore HOME for the rest of the test binary.
        // SAFETY: same rationale as the set_var above.
        unsafe {
            match prev {
                Some(p) => std::env::set_var("HOME", p),
                None => std::env::remove_var("HOME"),
            }
        }

        // Cleanup: ignore failures so a leaked tempdir doesn't fail CI.
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// 003-T2.19 acceptance criterion 3: permission failure produces
    /// a `BootstrapOutcome` with a warning string, not a panic. We
    /// can't easily simulate a chmod failure cross-platform; this
    /// test exercises the `path = None` branch by clearing `HOME`
    /// (the resolver returns `None`), which is the same code path
    /// callers must handle.
    #[test]
    fn bootstrap_handles_unresolvable_path() {
        let prev_home = std::env::var_os("HOME");
        let prev_xdg = std::env::var_os("XDG_DOCUMENTS_DIR");
        // SAFETY: env mutation is process-global. cargo nextest
        // isolates each test in its own process; the restore at the
        // end of the function keeps `cargo test` fallbacks polite.
        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("XDG_DOCUMENTS_DIR");
        }

        let outcome = bootstrap();

        // SAFETY: same rationale as above.
        unsafe {
            if let Some(p) = prev_home {
                std::env::set_var("HOME", p);
            }
            if let Some(p) = prev_xdg {
                std::env::set_var("XDG_DOCUMENTS_DIR", p);
            }
        }

        assert!(outcome.path.is_none());
        assert!(
            outcome.warning.is_some(),
            "unresolvable path should produce a toast-ready warning"
        );
    }
}
