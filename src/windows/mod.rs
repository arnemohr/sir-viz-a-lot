//! winit `ApplicationHandler`-driven window management. The output window
//! lives on the projector; the control window lives on the primary display.

// 004-P1.UX — renamed from `advanced` to `controls`; the panel now
// hosts both per-layer and project-level controls, so the
// "Advanced = expert-only" framing no longer fit.
#[cfg(feature = "v3")]
pub mod controls;
// 004-V31.9.2: audio bands strip — gated on both v3 and audio features so the
// module is entirely absent from the build graph when either is off.
pub mod anim;
#[cfg(all(feature = "v3", feature = "audio"))]
pub mod audio_bands_strip;
#[cfg(feature = "v3")]
pub mod components;
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
// 004-T1.24 — Look chain section: unified per-layer effect chain UI.
#[cfg(feature = "v3")]
pub mod look_chain;
// 004-T1.33 — once-per-machine onboarding toasts + UiFlags persistence.
#[cfg(feature = "v3")]
pub mod onboarding;
pub mod output;
#[cfg(feature = "v3")]
pub mod output_panel;
#[cfg(feature = "v3")]
pub mod preset_browser;
#[cfg(feature = "v3")]
pub mod preset_io;
#[cfg(feature = "v3")]
pub mod preset_stars;
pub mod primitives;
pub mod scene_editor;
#[cfg(feature = "v3")]
pub mod scene_io;
#[cfg(feature = "v3")]
pub mod show_day_strip;
pub mod theme;
#[cfg(feature = "v3")]
pub mod toast;
#[cfg(feature = "v3")]
pub mod toolbar;
#[cfg(feature = "v3")]
pub mod wizard;
