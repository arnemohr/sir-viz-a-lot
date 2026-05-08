//! 003-T1.41 — `ToastQueue`: in-process notification surface.
//!
//! Audit findings, save confirmations, and any other transient
//! operator-facing message land in this queue. The render side
//! (`toast_strip` in T1.42) walks `iter_visible(max)` each frame to
//! draw the top-N toasts in the canvas top-right corner.
//!
//! # TTL policy (open question D4)
//!
//! - `Info`: 4 s — auto-dismiss is friendly for routine confirmations
//!   ("Saved to /Users/.../show.rmap.json").
//! - `Warn`: 6 s — slightly longer so the operator notices and reads.
//! - `Error`: sticky (never expires) — operator must dismiss manually.
//!   Errors block the show; auto-dismissing them would mask problems.
//!
//! # Visibility cap
//!
//! `iter_visible(max)` shows at most `max` toasts. The default cap is
//! 3 — enough to convey most situations without burying the canvas.
//! A flood of toasts is itself a signal that something is wrong; the
//! UI doesn't try to show all of them.

#![deny(missing_docs)]
#![allow(dead_code)] // T-003-T1.42 wires `toast_strip`; T1.43 wires the audit driver.

use std::time::{Duration, Instant};

use crate::controls::Command;

/// Severity of a toast — drives styling (color / icon) and TTL policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// Routine confirmation. Auto-expires.
    Info,
    /// Warning: operator should look but the show isn't blocked.
    /// Auto-expires.
    Warn,
    /// Error: blocks the show. Sticky — operator dismisses manually.
    Error,
}

impl ToastKind {
    /// Default TTL for this kind. `None` means sticky.
    pub fn default_ttl(self) -> Option<Duration> {
        match self {
            ToastKind::Info => Some(Duration::from_secs(4)),
            ToastKind::Warn => Some(Duration::from_secs(6)),
            ToastKind::Error => None,
        }
    }
}

/// Optional action button on a toast. Click emits `command` through
/// the same dispatch path keyboard / MIDI / OSC use, so audit autofix
/// toasts can wire directly to `apply_command`.
#[derive(Debug, Clone)]
pub struct ToastAction {
    /// Button label shown on the toast.
    pub label: String,
    /// Command to dispatch on click. Reuses `controls::Command` so the
    /// existing `apply_command` machinery handles the click.
    pub command: Command,
}

/// One transient notification shown in the canvas top-right.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Severity / styling class.
    pub kind: ToastKind,
    /// User-facing message.
    pub message: String,
    /// Optional action button.
    pub action: Option<ToastAction>,
    /// Time-to-live. `None` means sticky (Error toasts).
    pub ttl: Option<Duration>,
    /// When this toast was pushed. Used by `drain_expired` to compute
    /// the deadline.
    pub created_at: Instant,
}

impl Toast {
    /// Construct an Info toast with the default 4 s TTL.
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Info, message)
    }

    /// Construct a Warn toast with the default 6 s TTL.
    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Warn, message)
    }

    /// Construct a sticky Error toast.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(ToastKind::Error, message)
    }

    /// Construct a toast of the given kind with the kind's default TTL.
    pub fn new(kind: ToastKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            action: None,
            ttl: kind.default_ttl(),
            created_at: Instant::now(),
        }
    }

    /// Attach an action button. Returns the modified toast for
    /// builder-style chaining: `Toast::warn(msg).with_action(...)`.
    pub fn with_action(mut self, action: ToastAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Has this toast outlived its TTL? Sticky toasts (`ttl == None`)
    /// always return `false`.
    fn is_expired(&self, now: Instant) -> bool {
        match self.ttl {
            Some(ttl) => now.duration_since(self.created_at) >= ttl,
            None => false,
        }
    }
}

/// Default visibility cap for `iter_visible`. The render side passes
/// this when it doesn't override.
pub const DEFAULT_VISIBLE_CAP: usize = 3;

/// In-process queue of [`Toast`]s. The audit driver / save buttons
/// `push`, the render side iterates with `iter_visible`, and the
/// app-loop calls `drain_expired` once per frame.
#[derive(Debug, Default)]
pub struct ToastQueue {
    toasts: Vec<Toast>,
}

impl ToastQueue {
    /// Construct an empty queue.
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    /// Add a toast to the queue. Order is FIFO — newer toasts append
    /// to the end. `iter_visible` reports them oldest-first; the
    /// render side flips for newest-on-top if desired.
    pub fn push(&mut self, toast: Toast) {
        self.toasts.push(toast);
    }

    /// Remove every toast whose TTL has elapsed. Sticky toasts (Error)
    /// are never removed by this method. Returns the number of toasts
    /// removed.
    pub fn drain_expired(&mut self) -> usize {
        let now = Instant::now();
        let before = self.toasts.len();
        self.toasts.retain(|t| !t.is_expired(now));
        before - self.toasts.len()
    }

    /// Iterate the most recent `max` toasts (newest-first). The render
    /// side calls this with `DEFAULT_VISIBLE_CAP` to avoid drowning
    /// the canvas in a flood of findings.
    pub fn iter_visible(&self, max: usize) -> impl Iterator<Item = &Toast> {
        self.toasts.iter().rev().take(max)
    }

    /// Number of toasts currently in the queue (visible + not yet
    /// drained).
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// `true` iff the queue holds no toasts.
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Drop a specific toast by index in insertion order. Used by the
    /// render-side dismiss button. No-op if `idx` is out of range.
    pub fn dismiss(&mut self, idx: usize) {
        if idx < self.toasts.len() {
            self.toasts.remove(idx);
        }
    }

    /// Drop every toast unconditionally. Used by the launcher when the
    /// project changes — old findings shouldn't carry over to a fresh
    /// load.
    pub fn clear(&mut self) {
        self.toasts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    /// 003-T1.41 AC#1: `push` adds a toast.
    #[test]
    fn push_adds_toast() {
        let mut q = ToastQueue::new();
        assert!(q.is_empty());
        q.push(Toast::info("hello"));
        assert_eq!(q.len(), 1);
    }

    /// 003-T1.41 AC#2: `drain_expired` removes toasts whose TTL elapsed.
    #[test]
    fn drain_expired_removes_only_expired_toasts() {
        let mut q = ToastQueue::new();

        // Push an Info toast with a tiny TTL so the test stays fast.
        let mut short = Toast::info("expires fast");
        short.ttl = Some(Duration::from_millis(20));
        q.push(short);
        // Push an Error toast (sticky).
        q.push(Toast::error("sticky"));
        // Push another Info with the default 4 s TTL — should NOT
        // expire during this test.
        q.push(Toast::info("survives"));
        assert_eq!(q.len(), 3);

        sleep(Duration::from_millis(50));
        let removed = q.drain_expired();
        assert_eq!(removed, 1, "only the 20 ms Info should expire");
        assert_eq!(q.len(), 2);
        // The sticky and the long-TTL Info both remain.
    }

    /// 003-T1.41 AC#3: `iter_visible(3)` returns at most 3 toasts.
    #[test]
    fn iter_visible_caps_at_max() {
        let mut q = ToastQueue::new();
        for i in 0..7 {
            q.push(Toast::info(format!("toast {i}")));
        }
        assert_eq!(q.iter_visible(3).count(), 3);
        // Asking for more than queue length returns the full queue.
        assert_eq!(q.iter_visible(99).count(), 7);
    }

    /// 003-T1.41 AC#4: sticky toasts (Error) never expire automatically.
    #[test]
    fn sticky_error_toasts_never_expire() {
        let mut q = ToastQueue::new();
        let err = Toast::error("blocking");
        assert_eq!(err.ttl, None, "Error TTL must be None (sticky)");
        q.push(err);

        sleep(Duration::from_millis(20));
        let removed = q.drain_expired();
        assert_eq!(removed, 0);
        assert_eq!(q.len(), 1, "sticky Error stays after drain_expired");
    }

    /// `iter_visible` reports newest-first so the render side can draw
    /// most-recent on top of older toasts in the canvas top-right.
    #[test]
    fn iter_visible_returns_newest_first() {
        let mut q = ToastQueue::new();
        q.push(Toast::info("first"));
        q.push(Toast::info("second"));
        q.push(Toast::info("third"));
        let labels: Vec<&str> = q.iter_visible(3).map(|t| t.message.as_str()).collect();
        assert_eq!(labels, vec!["third", "second", "first"]);
    }

    /// `dismiss` drops the toast at the given index. Out-of-range
    /// `idx` is a no-op.
    #[test]
    fn dismiss_drops_indexed_toast() {
        let mut q = ToastQueue::new();
        q.push(Toast::info("a"));
        q.push(Toast::info("b"));
        q.push(Toast::info("c"));
        q.dismiss(1);
        let labels: Vec<&str> = q.toasts.iter().map(|t| t.message.as_str()).collect();
        assert_eq!(labels, vec!["a", "c"]);

        // Out-of-range no-op.
        q.dismiss(99);
        assert_eq!(q.len(), 2);
    }

    /// `Toast::with_action` attaches a click action. Verifies the
    /// builder pattern for chaining.
    #[test]
    fn with_action_attaches_action() {
        let toast = Toast::warn("zero scale on layer 0").with_action(ToastAction {
            label: "Auto-fix".into(),
            command: Command::Blackout, // any Command works for the test
        });
        assert!(toast.action.is_some());
        assert_eq!(toast.action.as_ref().unwrap().label, "Auto-fix");
    }
}
