//! 004-T1.24 — Look chain section: unified per-layer effect chain UI.
//! Empty stub. Row rendering, drag-reorder, headline-param slider,
//! status dot, autofix chips, A/B compare arrive in T1.25–T1.29.

use egui::Ui;

use crate::project::schema::Project;
use crate::windows::control_panel::ControlPanelState;

/// 004-T1.24 — Look chain section. Empty stub; the full Look chain UI
/// (T1.25–T1.32) replaces this body.
pub fn show_look_chain_section(
    ui: &mut Ui,
    _project: &mut Project,
    _st: &mut ControlPanelState,
    _layer_idx: usize,
) {
    ui.label("Look chain — coming soon (T1.25+)");
}
