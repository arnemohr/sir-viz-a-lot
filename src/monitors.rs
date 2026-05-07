//! Monitor enumeration. Wraps winit's monitor list into a stable, owned shape
//! the rest of the app can hold across event-loop iterations.
//!
//! Note on macOS: T-M1-01 originally called for an `objc2-app-kit` `NSScreen`
//! fallback whenever `MonitorHandle::name()` returns `None`. In winit 0.30,
//! the macOS impl unconditionally returns `Some(format!("Monitor #{n}"))`, so
//! the `None` branch is unreachable on that platform. Per the task's
//! 30-minute escape hatch, the `NSScreen` integration was dropped; the
//! generic numeric fallback below covers any future winit version that
//! changes that behaviour.
//!
//! See `specs/001-tasks.md` (T-M1-01) and `specs/001-initial-setup-plan.md`
//! §3.4 M1 deltas.

use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;

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
}

impl MonitorInfo {
    fn from_handle(index: usize, handle: &MonitorHandle) -> Self {
        let position = handle.position();
        let name = handle.name().unwrap_or_else(|| {
            tracing::warn!(
                index,
                position = ?position,
                "monitor name unavailable; falling back to default",
            );
            format!("Monitor {index}")
        });
        let size = handle.size();
        Self {
            index,
            name,
            size: (size.width, size.height),
            position: (position.x, position.y),
            scale_factor: handle.scale_factor(),
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
pub fn primary(active_loop: &ActiveEventLoop) -> Option<MonitorInfo> {
    let primary = active_loop.primary_monitor()?;
    let index = active_loop
        .available_monitors()
        .position(|m| m == primary)
        .unwrap_or(0);
    Some(MonitorInfo::from_handle(index, &primary))
}
