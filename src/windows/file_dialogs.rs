//! Native OS file pickers for the launcher and editor flows.
//!
//! 003-T2.13 — wraps the [`rfd`] crate so the rest of the codebase
//! talks to a small set of typed helpers instead of building a
//! `FileDialog` inline at every call site. Each helper:
//!
//! - returns `Option<PathBuf>` so a Cancel click is the same shape
//!   as a successful pick;
//! - applies the right MIME / extension filter for the flow;
//! - sets a sensible window title (`"Add image"`, `"Save show as…"`,
//!   `"Open show"`).
//!
//! The dialogs are *blocking* — `rfd::FileDialog::pick_file` runs the
//! native panel modally, which means the calling thread (the winit
//! main thread) is parked while the dialog is up. That's intentional
//! for an operator triggering a one-shot picker; the layer canvas
//! isn't expected to render mid-dialog. The trade-off is documented
//! per call site (see T-003-T2.24's note on the relink flow).

use std::path::{Path, PathBuf};

use rfd::FileDialog;

/// 003-T2.13 — pick an image file (JPG / PNG / SVG) to add as a new
/// layer.
///
/// The `+ Add image` button (T-003-T2.14) and the menu fallback both
/// route through here. Returns `None` on cancel.
#[allow(dead_code)] // Wired by T-003-T2.14 (+ Add image button) and T-003-T2.10 (Open Recent fallback).
pub fn pick_image_to_add() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Add image")
        .add_filter("Images (JPG, PNG, SVG)", &["jpg", "jpeg", "png", "svg"])
        .pick_file()
}

/// 003-T2.13 — pick a destination for `Save as…`. Suggests `default_name`
/// (the operator's working filename) and ensures the result ends in
/// `.rmap.json` regardless of what they typed.
///
/// rfd handles the platform-specific extension nudge (NSSavePanel
/// auto-appends; some Linux portals do not) — we always append the
/// suffix as a defensive belt-and-braces step. If the operator types
/// `"my show.rmap.json"` and macOS appends nothing, the path is
/// already fine; if they type `"my show"` and the portal returns it
/// verbatim, [`ensure_rmap_extension`] adds the suffix.
#[allow(dead_code)] // Wired by T-003-T2.* Save-as flow once it ships.
pub fn pick_save_destination(default_name: &str) -> Option<PathBuf> {
    let path = FileDialog::new()
        .set_title("Save show as…")
        .add_filter("rmap project (.rmap.json)", &["rmap.json"])
        .set_file_name(default_name)
        .save_file()?;
    Some(ensure_rmap_extension(path))
}

/// 003-T2.13 — pick an existing `.rmap.json` to open.
///
/// Used as the launcher's "Open recent" alternative path when the
/// operator wants a project that isn't in `~/Documents/rmap/`.
#[allow(dead_code)] // Wired by T-003-T2.10 (launcher Open-Recent alternative).
pub fn pick_open_project() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Open show")
        .add_filter("rmap project (.rmap.json)", &["rmap.json"])
        .pick_file()
}

/// Append `.rmap.json` to `path` unless it already ends in that
/// suffix. Pulled out so the suffix policy stays consistent across
/// callers and is unit-testable without an OS dialog.
///
/// We compare against the full `.rmap.json` suffix rather than just
/// `.json` because operators sometimes type `foo.json` intending a
/// generic JSON file; appending `.rmap.json` to that gives
/// `foo.json.rmap.json`, which is ugly but correct — `Project::load`
/// only opens files matching `.rmap.json`.
fn ensure_rmap_extension(path: PathBuf) -> PathBuf {
    if path_ends_with_rmap_json(&path) {
        path
    } else {
        let mut s = path.into_os_string();
        s.push(".rmap.json");
        PathBuf::from(s)
    }
}

fn path_ends_with_rmap_json(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|s| s.to_ascii_lowercase().ends_with(".rmap.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_extension_passes_through_when_already_correct() {
        let p = PathBuf::from("/tmp/show.rmap.json");
        assert_eq!(ensure_rmap_extension(p.clone()), p);

        let upper = PathBuf::from("/tmp/SHOW.RMAP.JSON");
        assert_eq!(ensure_rmap_extension(upper.clone()), upper);
    }

    #[test]
    fn ensure_extension_appends_when_missing() {
        let p = PathBuf::from("/tmp/show");
        assert_eq!(
            ensure_rmap_extension(p),
            PathBuf::from("/tmp/show.rmap.json")
        );

        // Operators sometimes type the wrong extension; we keep the
        // typed text intact and append, so the resulting filename is
        // unambiguous about which loader can open it.
        let json = PathBuf::from("/tmp/show.json");
        assert_eq!(
            ensure_rmap_extension(json),
            PathBuf::from("/tmp/show.json.rmap.json")
        );
    }
}
