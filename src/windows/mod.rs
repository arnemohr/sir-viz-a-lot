//! winit `ApplicationHandler`-driven window management. The output window
//! lives on the projector; the control window lives on the primary display.

pub mod control;
pub mod control_panel;
#[cfg(feature = "v3")]
pub mod launcher;
pub mod output;
pub mod scene_editor;
#[cfg(feature = "v3")]
pub mod toast;
