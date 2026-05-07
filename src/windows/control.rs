//! egui-backed control window. Sliders bind to layer/effect/modulator
//! parameters; a Mapping tab edits the warp mesh corners; a Scenes tab
//! manages snapshots.
//!
//! TODO(M4-14): egui-winit + egui-wgpu integration. Render into a separate
//! winit Window on the primary display.

/// Push a sticky error message onto the control-window error overlay so
/// the operator sees it without having to read the log file. Currently a
/// stub: T-M4-14 will fold this into the egui control window. Until then
/// the function exists so T-M2-10's `App::window_event` panic-recovery arm
/// has a known consumer; the message is also routed to `tracing::error!`
/// so the operator sees it on stderr today.
///
/// Spec ref: §6 Show-day requirements row "Error overlay" — this overlay
/// must live on the control window, NEVER on the output (which would
/// project an error message at the wedding).
pub fn error_overlay(msg: &str) {
    tracing::error!(
        msg = msg,
        "error overlay (egui rendering deferred to T-M4-14)"
    );
}
