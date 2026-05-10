//! winit `ApplicationHandler`-driven window management. The output window
//! lives on the projector; the control window lives on the primary display.

#[cfg(feature = "v3")]
pub mod advanced;
// 004-V31.9.2: audio bands strip — gated on both v3 and audio features so the
// module is entirely absent from the build graph when either is off.
pub mod anim;
#[cfg(all(feature = "v3", feature = "audio"))]
pub mod audio_bands_strip;
pub mod control;
pub mod control_panel;
#[cfg(feature = "v3")]
pub mod cue_strip;
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
pub mod show_day_strip;
pub mod theme;
#[cfg(feature = "v3")]
pub mod toast;
#[cfg(feature = "v3")]
pub mod toolbar;
