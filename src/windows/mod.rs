//! winit `ApplicationHandler`-driven window management. The output window
//! lives on the projector; the control window lives on the primary display.

#[cfg(feature = "v3")]
pub mod advanced;
pub mod control;
pub mod control_panel;
#[cfg(feature = "v3")]
pub mod file_dialogs;
#[cfg(feature = "v3")]
pub mod glossary;
#[cfg(feature = "v3")]
pub mod inspector;
#[cfg(feature = "v3")]
pub mod launcher;
#[cfg(feature = "v3")]
pub mod layer_strip;
pub mod output;
#[cfg(feature = "v3")]
pub mod primitives;
pub mod scene_editor;
#[cfg(feature = "v3")]
pub mod toast;
#[cfg(feature = "v3")]
pub mod toolbar;
