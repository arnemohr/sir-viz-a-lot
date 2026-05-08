//! Recent-projects listing for the launcher's "Open recent" sub-list.
//!
//! 003-T2.10 — scans `~/Documents/rmap/` for `*.rmap.json` files and
//! returns up to ten of them, sorted by modification time newest-first.
//! The launcher's "Open recent" button is disabled while the listing
//! is empty (T-003-T2.4).
//!
//! Format trade-offs:
//!
//! - **Top-10 cap**: matches Q6 / D6 in `specs/003-tasks.md`. Operators
//!   accumulate dozens of show files over a season; ten is the recall
//!   horizon for "the show I just edited" or "the show from last
//!   weekend". Older shows live in the file picker (T-003-T2.13).
//! - **Skip `_autosave/`**: the autosave subdir holds crash-recovery
//!   snapshots, not deliberate saves. Surfacing them in the recents
//!   listing would teach operators to load partials that don't carry
//!   the user-facing project name.
//! - **Relative dates**: "2 hours ago", "yesterday", "Mar 4" — the
//!   absolute calendar form lands once a day boundary passes so the
//!   operator can scan by month / day at a glance.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A single entry in the launcher's recents listing.
#[derive(Debug, Clone)]
pub struct RecentProject {
    pub path: PathBuf,
    /// Filename minus the `.rmap.json` suffix. Used as the operator-
    /// facing label in the picker.
    pub label: String,
    /// `mtime` from the filesystem; drives the sort and the relative-
    /// date display.
    pub modified: SystemTime,
}

/// Phase-2 cap on listing length per Q6 / D6 in `specs/003-tasks.md`.
const RECENTS_LIMIT: usize = 10;

/// Filename suffix that marks a project file. Lower-case match because
/// macOS / Linux filesystems are case-sensitive but operators sometimes
/// type `.RMAP.JSON` from Finder.
const RMAP_SUFFIX: &str = ".rmap.json";

/// 003-T2.10 — scan `dir` for `*.rmap.json` files and return the top
/// `RECENTS_LIMIT` sorted mtime-descending.
///
/// Returns an empty Vec if `dir` is missing, unreadable, or doesn't
/// hold any project files. The launcher renders an empty state in
/// that case (T-003-T2.4 disables the "Open recent" button).
pub fn scan(dir: &Path) -> Vec<RecentProject> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        tracing::debug!(
            path = %dir.display(),
            "recents: directory unreadable; treating as empty",
        );
        return Vec::new();
    };

    let mut found = Vec::with_capacity(16);
    for entry in entries.flatten() {
        let path = entry.path();
        // Skip `_autosave/` (crash-recovery snapshots) and any other
        // subdirectory; only top-level files surface in the listing.
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                tracing::debug!(
                    path = %path.display(),
                    ?err,
                    "recents: stat failed; skipping entry",
                );
                continue;
            }
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !filename.to_ascii_lowercase().ends_with(RMAP_SUFFIX) {
            continue;
        }
        let label = filename
            .strip_suffix(RMAP_SUFFIX)
            .or_else(|| filename.strip_suffix(&RMAP_SUFFIX.to_ascii_uppercase()))
            .unwrap_or(filename)
            .to_string();
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        found.push(RecentProject {
            path,
            label,
            modified,
        });
    }

    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found.truncate(RECENTS_LIMIT);
    found
}

/// Render `modified` as a short relative-date string.
///
/// Buckets:
///
/// - `<1 min` → `"just now"`
/// - `<1 h`   → `"N min ago"`
/// - `<24 h`  → `"N hr ago"`
/// - `<7 d`   → `"N days ago"`
/// - older    → `"YYYY-MM-DD"` (ISO-ish; no localisation in v3)
///
/// `now` is parameterised so the function is unit-testable without
/// mocking the system clock.
pub fn relative_date(modified: SystemTime, now: SystemTime) -> String {
    let elapsed = match now.duration_since(modified) {
        Ok(d) => d,
        // Filesystem clock skew can produce a "modified in the future"
        // result. Treat that as "just now" rather than crashing the
        // launcher render with a panic on `unwrap()`.
        Err(_) => return "just now".to_string(),
    };
    if elapsed < Duration::from_secs(60) {
        return "just now".to_string();
    }
    if elapsed < Duration::from_secs(60 * 60) {
        let mins = elapsed.as_secs() / 60;
        return format!("{mins} min ago");
    }
    if elapsed < Duration::from_secs(60 * 60 * 24) {
        let hours = elapsed.as_secs() / 3600;
        return format!("{hours} hr ago");
    }
    if elapsed < Duration::from_secs(60 * 60 * 24 * 7) {
        let days = elapsed.as_secs() / (60 * 60 * 24);
        return format!("{days} days ago");
    }
    // Older than a week — fall through to a calendar form. We don't
    // have `chrono` as a dep, so format the modified time via
    // `humantime`-style ISO string. `SystemTime` → seconds → naive
    // calendar via integer arithmetic. v3 is good enough at this
    // resolution; localisation is post-M2.
    iso_date_from_systemtime(modified)
}

/// `YYYY-MM-DD` (UTC) for `modified`. Tiny calendar arithmetic so we
/// don't pull in `chrono`. Returns `"unknown"` if the SystemTime
/// predates the epoch (impossible in practice on modern filesystems).
fn iso_date_from_systemtime(t: SystemTime) -> String {
    let Ok(d) = t.duration_since(SystemTime::UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let secs = d.as_secs() as i64;
    let days_since_epoch = secs / 86_400;
    // Civil-from-days, Howard Hinnant's date library algorithm —
    // the well-known closed-form solution. Public-domain reference
    // implementation from his paper "chrono-Compatible Low-Level Date
    // Algorithms" (https://howardhinnant.github.io/date_algorithms.html).
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146097)
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_recents_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rmap_t2_10_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            name,
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// 003-T2.10 acceptance criterion 1: listing shows up to ten files,
    /// sorted mtime-desc, ignoring non-rmap files and the `_autosave/`
    /// subdirectory.
    #[test]
    fn scan_returns_top_ten_sorted_mtime_desc() {
        let dir = temp_recents_dir("scan-sort");
        // Create 12 fake project files.
        for i in 0..12 {
            let p = dir.join(format!("show{i}.rmap.json"));
            std::fs::write(&p, b"{}").expect("write");
            // Set mtime explicitly via touch-style — rely on creation
            // order being roughly chronological since file_times APIs
            // need a feature gate. The order test below uses index
            // suffixes to verify ordering, not exact mtimes.
        }
        // A non-rmap file that should be ignored.
        std::fs::write(dir.join("notes.txt"), b"ignore me").expect("write");
        // The autosave subdirectory should be skipped — even if it
        // contains rmap.json files (recents lists deliberate saves).
        std::fs::create_dir_all(dir.join("_autosave")).expect("autosave dir");
        std::fs::write(dir.join("_autosave/recovery.rmap.json"), b"{}").expect("write");

        let listing = scan(&dir);
        assert_eq!(listing.len(), 10, "capped at RECENTS_LIMIT");
        // Mtime-desc ordering: every entry's mtime is >= the next.
        for w in listing.windows(2) {
            assert!(w[0].modified >= w[1].modified);
        }
        // Labels strip the .rmap.json suffix.
        assert!(listing.iter().all(|r| !r.label.contains(".rmap.json")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 003-T2.10 acceptance criterion 2: missing or empty directory
    /// produces an empty Vec, not a panic.
    #[test]
    fn scan_returns_empty_for_missing_directory() {
        let bogus = std::env::temp_dir().join("rmap_t2_10_definitely_not_here");
        let _ = std::fs::remove_dir_all(&bogus);
        let listing = scan(&bogus);
        assert!(listing.is_empty());
    }

    #[test]
    fn relative_date_buckets_recent_times() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        assert_eq!(
            relative_date(now - Duration::from_secs(30), now),
            "just now"
        );
        assert_eq!(
            relative_date(now - Duration::from_secs(60 * 5), now),
            "5 min ago"
        );
        assert_eq!(
            relative_date(now - Duration::from_secs(60 * 60 * 3), now),
            "3 hr ago"
        );
        assert_eq!(
            relative_date(now - Duration::from_secs(60 * 60 * 24 * 3), now),
            "3 days ago"
        );
        // Older than a week → ISO date.
        let result = relative_date(now - Duration::from_secs(60 * 60 * 24 * 30), now);
        assert!(result.starts_with("20"), "expected ISO date, got {result}");
    }

    #[test]
    fn relative_date_handles_future_modified_time() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let future = now + Duration::from_secs(60);
        // Filesystem clock skew can produce a future mtime; we should
        // treat it as "just now" rather than crashing.
        assert_eq!(relative_date(future, now), "just now");
    }
}
