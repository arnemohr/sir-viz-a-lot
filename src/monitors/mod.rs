//! Monitor enumeration. Wraps winit's monitor list into a stable, owned shape
//! the rest of the app can hold across event-loop iterations.
//!
//! On macOS the human-readable display names (e.g. `"BenQ TH685"`) come from
//! `NSScreen::localizedName`; winit 0.30 returns numeric placeholders like
//! `"Monitor #41052"`. The cross-platform `display_name` helper below picks
//! the right source per OS — see `macos` (this module) and the trivial
//! fallback for Linux / Windows.
//!
//! See `specs/003-tasks-phase-2.md` (T-003-T2.7) and
//! `specs/001-tasks.md` (T-M1-01).

#[cfg(target_os = "macos")]
mod macos;

use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
#[cfg(target_os = "macos")]
use winit::platform::macos::MonitorHandleExtMacOS;

/// Owned snapshot of a single monitor, decoupled from winit's `MonitorHandle`
/// so consumers (project schema, UI dropdown) can store these in plain data
/// structures.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub size: (u32, u32),
    pub position: (i32, i32),
    pub scale_factor: f64,
    /// 003-T2.7 — platform-stable identifier for the display, when one is
    /// available. On macOS this is the `CGDirectDisplayID` (a.k.a.
    /// `NSScreenNumber`), printed as a decimal string. Other platforms
    /// return `None` until a stable id source lands. T-003-T2.20 stores
    /// this value in `~/Library/Preferences/rmap.toml` so the projector
    /// dropdown can prefill the operator's last-used display across
    /// sessions.
    #[allow(dead_code)] // Read by T-003-T2.20 (last-used-projector prefs).
    pub stable_id: Option<String>,
    /// V31.2.2 — cross-machine UUID from `CGDisplayCreateUUIDFromDisplayID`.
    /// `None` until V31.2.3 implements the macOS lookup. Other platforms
    /// will likely stay `None` until they grow an equivalent.
    pub uuid: Option<String>,
}

impl MonitorInfo {
    fn from_handle(index: usize, handle: &MonitorHandle) -> Self {
        let position = handle.position();
        let size = handle.size();
        let name = display_name(handle, index);
        let stable_id = stable_id(handle);
        // V31.2.3 — populate UUID from CGDisplayCreateUUIDFromDisplayID on macOS.
        // Other platforms return None (no equivalent stable cross-machine UUID
        // API is available without OS-specific plumbing).
        #[cfg(target_os = "macos")]
        let uuid = {
            let id = handle.native_id();
            macos::uuid_for_display_id(id)
        };
        #[cfg(not(target_os = "macos"))]
        let uuid = None;

        Self {
            index,
            name,
            size: (size.width, size.height),
            position: (position.x, position.y),
            scale_factor: handle.scale_factor(),
            stable_id,
            uuid,
        }
    }
}

/// Enumerate every monitor winit reports, in the same order.
///
/// Must be called from inside an `ApplicationHandler` callback (e.g. on
/// `resumed`); the `ActiveEventLoop` reference is the only legal place to
/// query `available_monitors()` in winit 0.30.
pub fn list(active_loop: &ActiveEventLoop) -> Vec<MonitorInfo> {
    active_loop
        .available_monitors()
        .enumerate()
        .map(|(index, handle)| MonitorInfo::from_handle(index, &handle))
        .collect()
}

/// Look up the platform's primary monitor, if any.
///
/// The returned `MonitorInfo::index` is the position of the primary in
/// `available_monitors()` so it can round-trip into `Project.output_monitor_index`.
/// If the primary handle is not findable in the iterator (rare; observed on
/// some hot-plug edge cases), the index falls back to `0`.
#[allow(dead_code)] // T-M6-04: wired in when output_monitor_index autoselect lands
pub fn primary(active_loop: &ActiveEventLoop) -> Option<MonitorInfo> {
    let primary = active_loop.primary_monitor()?;
    let index = active_loop
        .available_monitors()
        .position(|m| m == primary)
        .unwrap_or(0);
    Some(MonitorInfo::from_handle(index, &primary))
}

/// 003-T2.7 — pick a human-readable name for a monitor.
///
/// On macOS, `NSScreen::localizedName()` (via the [`macos`] submodule)
/// returns the user-facing display name — `"Built-in Display"`,
/// `"BenQ TH685"`, `"Living Room Wall"`, etc. winit's
/// `MonitorHandle::name()` on macOS returns `"Monitor #41052"`-style
/// placeholders that aren't recognisable.
///
/// On Linux / Windows the macOS branch is gated out and `handle.name()`
/// passes through; the numeric fallback (`"Display N"`) catches the case
/// where winit reports `None`.
fn display_name(handle: &MonitorHandle, fallback_idx: usize) -> String {
    #[cfg(target_os = "macos")]
    {
        let id = handle.native_id();
        if let Some(name) = macos::localized_name_for_display_id(id) {
            return name;
        }
    }
    handle.name().unwrap_or_else(|| {
        tracing::warn!(
            index = fallback_idx,
            "monitor name unavailable; falling back to default",
        );
        format!("Display {fallback_idx}")
    })
}

/// 003-T2.7 — platform-stable display identifier for prefs persistence.
///
/// macOS returns the `CGDirectDisplayID` (the same value Apple's
/// `kCGDirectMainDisplay` and `NSScreenNumber` keys produce) printed as
/// a decimal `String`. The value survives unplug+replug as long as the
/// display itself doesn't change identifier — which is the contract the
/// `last_used_projector_uuid` pref relies on (T-003-T2.20).
///
/// Linux / Windows return `None` until a stable-id source lands. The
/// dropdown still works; it just can't preselect the previous projector.
fn stable_id(handle: &MonitorHandle) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        Some(handle.native_id().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = handle;
        None
    }
}

/// Outcome of resolving an `OutputTarget` against live monitors.
///
/// V31.2.2 — callers use this instead of indexing the monitor list directly
/// so UUID matching (V31.2.3) is automatically preferred when available.
#[derive(Debug, Clone)]
pub enum ResolveOutcome {
    /// UUID matched a live monitor.
    UuidMatch(MonitorInfo),
    /// UUID absent or didn't match; index resolved cleanly.
    IndexMatch(MonitorInfo),
    /// Both UUID and index failed; fell back to display 0 with a
    /// non-fatal audit warning.
    Fallback {
        selected: MonitorInfo,
        reason: ResolveFallbackReason,
    },
}

impl ResolveOutcome {
    /// The resolved monitor, regardless of which path was taken.
    pub fn monitor(&self) -> &MonitorInfo {
        match self {
            ResolveOutcome::UuidMatch(m) | ResolveOutcome::IndexMatch(m) => m,
            ResolveOutcome::Fallback { selected, .. } => selected,
        }
    }
}

/// Reason the resolution fell back to display 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveFallbackReason {
    /// `OutputTarget.uuid` was Some but no live monitor matched.
    ///
    /// Note: this variant fires only when `fallback_index` is also valid and
    /// the caller explicitly wants to surface a warning despite a successful
    /// index match. The current `resolve_output_target` returns `IndexMatch`
    /// (not `Fallback`) when UUID fails but index succeeds; this variant is
    /// reserved for future use (e.g. explicit audit-warning-only paths).
    #[allow(dead_code)]
    UuidNotFound,
    /// `OutputTarget.fallback_index` was out of range vs live monitor count.
    IndexOutOfRange,
    /// UUID was Some, no match, and `fallback_index` was also out of range.
    /// (Both rules failed.)
    UuidAndIndexBothMissing,
}

/// Resolve an `OutputTarget` to a live `MonitorInfo`.
///
/// Precedence:
/// 1. If `target.uuid` is `Some(u)` **and** a `MonitorInfo` has `uuid == Some(u)`,
///    return `UuidMatch`.
/// 2. Else, if `target.fallback_index < monitors.len()`, return
///    `IndexMatch(monitors[target.fallback_index])`.
/// 3. Else, return `Fallback { selected: monitors[0], reason }`.
///
/// Panics if `monitors` is empty — rmap requires ≥1 display.
pub fn resolve_output_target(
    target: &crate::project::schema::OutputTarget,
    monitors: &[MonitorInfo],
) -> ResolveOutcome {
    debug_assert!(
        !monitors.is_empty(),
        "resolve_output_target: monitor list must not be empty"
    );

    // Step 1 — UUID match.
    if let Some(ref uuid) = target.uuid {
        if let Some(m) = monitors
            .iter()
            .find(|m| m.uuid.as_deref() == Some(uuid.as_str()))
        {
            return ResolveOutcome::UuidMatch(m.clone());
        }
        // UUID was set but not found in the live list.
        // Still attempt the index fallback before giving up.
        if target.fallback_index < monitors.len() {
            // UUID miss but index is valid — treat as IndexMatch (UUID just not live).
            return ResolveOutcome::IndexMatch(monitors[target.fallback_index].clone());
        }
        // Both UUID and index failed.
        return ResolveOutcome::Fallback {
            selected: monitors[0].clone(),
            reason: ResolveFallbackReason::UuidAndIndexBothMissing,
        };
    }

    // Step 2 — no UUID; use fallback_index.
    if target.fallback_index < monitors.len() {
        return ResolveOutcome::IndexMatch(monitors[target.fallback_index].clone());
    }

    // Step 3 — index out of range.
    ResolveOutcome::Fallback {
        selected: monitors[0].clone(),
        reason: ResolveFallbackReason::IndexOutOfRange,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::OutputTarget;

    /// 003-T2.7 acceptance criterion 1 + 4: on macOS, looking up an id
    /// that does not match any attached display falls back to `None`
    /// (the same code path the spec calls out as "fail to match" in
    /// the bullet list). Tests run inside `cargo test` may not have a
    /// connected display at all; the fallback path needs to remain
    /// panic-free in that case.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_localized_name_unknown_id_returns_none() {
        // Display id 0 is reserved and never appears in
        // `NSScreen::screens()`. The match-by-id loop should bottom
        // out at `None`.
        let result = super::macos::localized_name_for_display_id(0);
        assert!(
            result.is_none(),
            "unknown display id should yield None, not a panic or an empty string"
        );
    }

    // --- V31.2.2: resolve_output_target tests ---

    fn mk_monitor(index: usize, uuid: Option<&str>) -> MonitorInfo {
        MonitorInfo {
            index,
            name: format!("Display {index}"),
            size: (1920, 1080),
            position: (0, 0),
            scale_factor: 1.0,
            stable_id: None,
            uuid: uuid.map(|s| s.to_string()),
        }
    }

    /// V31.2.2 — UUID present in both target and live monitor → UuidMatch.
    #[test]
    fn resolve_uuid_match_success() {
        let monitors = vec![mk_monitor(0, None), mk_monitor(1, Some("AAAA-1111"))];
        let target = OutputTarget {
            uuid: Some("AAAA-1111".to_string()),
            fallback_index: 0,
            ..OutputTarget::default()
        };
        let outcome = resolve_output_target(&target, &monitors);
        assert!(
            matches!(outcome, ResolveOutcome::UuidMatch(ref m) if m.index == 1),
            "expected UuidMatch for index 1, got {outcome:?}",
        );
    }

    /// V31.2.2 — UUID Some but no live monitor has it; index is valid → IndexMatch.
    #[test]
    fn resolve_uuid_some_no_match_index_valid() {
        let monitors = vec![mk_monitor(0, None), mk_monitor(1, Some("BBBB-2222"))];
        let target = OutputTarget {
            uuid: Some("ZZZZ-9999".to_string()), // not in live list
            fallback_index: 1,
            ..OutputTarget::default()
        };
        let outcome = resolve_output_target(&target, &monitors);
        assert!(
            matches!(outcome, ResolveOutcome::IndexMatch(ref m) if m.index == 1),
            "expected IndexMatch for index 1 (UUID miss fallback), got {outcome:?}",
        );
    }

    /// V31.2.2 — UUID None, valid fallback_index → IndexMatch.
    #[test]
    fn resolve_uuid_none_index_valid() {
        let monitors = vec![mk_monitor(0, None), mk_monitor(1, None)];
        let target = OutputTarget {
            uuid: None,
            fallback_index: 1,
            ..OutputTarget::default()
        };
        let outcome = resolve_output_target(&target, &monitors);
        assert!(
            matches!(outcome, ResolveOutcome::IndexMatch(ref m) if m.index == 1),
            "expected IndexMatch for index 1, got {outcome:?}",
        );
    }

    /// V31.2.2 — UUID None, fallback_index out of range → Fallback(IndexOutOfRange).
    #[test]
    fn resolve_uuid_none_index_out_of_range() {
        let monitors = vec![mk_monitor(0, None)];
        let target = OutputTarget {
            uuid: None,
            fallback_index: 5,
            ..OutputTarget::default()
        };
        let outcome = resolve_output_target(&target, &monitors);
        assert!(
            matches!(
                outcome,
                ResolveOutcome::Fallback {
                    ref selected,
                    reason: ResolveFallbackReason::IndexOutOfRange
                } if selected.index == 0
            ),
            "expected Fallback(IndexOutOfRange) to monitor 0, got {outcome:?}",
        );
    }

    /// V31.2.2 — UUID Some, no match, AND fallback_index out of range →
    /// Fallback(UuidAndIndexBothMissing).
    #[test]
    fn resolve_uuid_some_no_match_index_out_of_range() {
        let monitors = vec![mk_monitor(0, None)];
        let target = OutputTarget {
            uuid: Some("ZZZZ-9999".to_string()),
            fallback_index: 5,
            ..OutputTarget::default()
        };
        let outcome = resolve_output_target(&target, &monitors);
        assert!(
            matches!(
                outcome,
                ResolveOutcome::Fallback {
                    ref selected,
                    reason: ResolveFallbackReason::UuidAndIndexBothMissing
                } if selected.index == 0
            ),
            "expected Fallback(UuidAndIndexBothMissing) to monitor 0, got {outcome:?}",
        );
    }
}
