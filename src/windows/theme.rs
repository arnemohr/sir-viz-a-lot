//! Central egui theme tokens and theme hook for rmap's windows.
//!
//! Both the control window (`windows::control`) and the launcher window
//! (`windows::launcher`) call [`install`] on their `egui::Context` after
//! creation so any future style change — colours, spacing, fonts,
//! pixels-per-point, dark/light mode — lives in **one place** instead of
//! drifting across two `Context::default()` call sites.
//!
//! # Colour system
//!
//! One warm accent (gold), low-saturation panel backgrounds, secondary
//! text grey, distinct warning/destructive/success semantics.  WCAG AA
//! contrast: `TEXT_PRIMARY` (220, 220, 220) on `BG_PANEL` (20, 22, 28)
//! is approximately 10:1 — well above the 4.5:1 minimum.
//!
//! ## Colour constants
//!
//! Constants are `pub` and not `#[cfg(feature = "v3")]`-gated: both v2
//! and v3 paint paths may import them.  The *migration of existing
//! literals* to these constants is scoped to v3 paint sites.

// Theme constants are intentionally ungated (see module doc) so they are
// available to both v2 and v3 paint paths. When v3 is inactive most of them
// have no call sites, so suppress the resulting dead_code lint globally for
// this module rather than cluttering every constant declaration.
#![cfg_attr(not(feature = "v3"), allow(dead_code))]

// ── Background ───────────────────────────────────────────────────────────────

pub const BG_BACKGROUND: egui::Color32 = egui::Color32::from_rgb(8, 9, 12);
pub const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(20, 22, 28);

// ── Text ─────────────────────────────────────────────────────────────────────

pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(220, 220, 220);
pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_gray(140);

// ── Accent ───────────────────────────────────────────────────────────────────

/// Warm gold — the primary interactive accent.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(255, 200, 100);
/// Dimmed warm gold — used for unselected handles and secondary accent hits.
pub const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(180, 140, 70);

// ── Semantic status ───────────────────────────────────────────────────────────

#[allow(dead_code)] // reserved for forthcoming warning-toast surfaces
pub const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 180, 80);
pub const DESTRUCTIVE: egui::Color32 = egui::Color32::from_rgb(220, 100, 100);
pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(120, 200, 140);

// ── Canvas interaction handles ────────────────────────────────────────────────

/// Unselected warp handle / mask vertex / drag-source marker.
pub const HANDLE_DEFAULT: egui::Color32 = ACCENT_DIM;
/// Selected / active warp handle / mask vertex.
pub const HANDLE_ACTIVE: egui::Color32 = ACCENT;

// ── Canvas overlays ───────────────────────────────────────────────────────────

/// Warp mesh grid lines — faint cool-tinted stroke.
pub const MESH_LINE: egui::Color32 = egui::Color32::from_rgba_premultiplied(160, 200, 255, 90);
/// Selected-layer outline — warm accent so it reads as "mine".
#[allow(dead_code)] // reserved for the M4 selected-layer outline (UX punch-list item)
pub const SELECTED_OUTLINE: egui::Color32 = ACCENT;
/// Mask polygon edge — violet so it's distinct from both the accent and
/// the neutral layer-outline palette.
pub const MASK_EDGE: egui::Color32 = egui::Color32::from_rgb(140, 100, 200);
/// Dark outline stroke drawn behind unselected handle dots so they pop
/// against any canvas content colour.
pub const HANDLE_OUTLINE: egui::Color32 = egui::Color32::from_gray(40);

// ── Per-layer outline palette ─────────────────────────────────────────────────

/// 8-entry deterministic colour palette for layer bounding-box outlines.
/// Cycles by layer index; same values as used in `scene_editor::layer_color`
/// and `render::overlay::layer_color` (kept in those modules for the
/// `[f32; 4]` path).
pub const LAYER_PALETTE: [egui::Color32; 8] = [
    egui::Color32::from_rgb(255, 110, 130), // pink
    egui::Color32::from_rgb(110, 200, 255), // sky
    egui::Color32::from_rgb(180, 240, 130), // lime
    egui::Color32::from_rgb(255, 200, 90),  // amber
    egui::Color32::from_rgb(190, 130, 245), // violet
    egui::Color32::from_rgb(110, 230, 200), // teal
    egui::Color32::from_rgb(245, 150, 80),  // orange
    egui::Color32::from_rgb(180, 180, 220), // grey-violet
];

// ── Theme hook ────────────────────────────────────────────────────────────────

/// Apply rmap's egui theme to `ctx`. Call once, immediately after
/// `egui::Context::default()`.
///
/// Overrides egui's built-in dark `Visuals` with rmap's colour tokens so
/// every panel, window, and widget inherits the palette without per-widget
/// colour overrides.
pub fn install(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.extreme_bg_color = BG_BACKGROUND;

    // Override the text colour for non-interactive widgets (labels, etc.).
    visuals.override_text_color = Some(TEXT_PRIMARY);

    // Selection highlight — translucent accent.
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    // Widget fill colours: use slightly-lighter-than-panel to give buttons
    // and inputs a visible surface without blowing out the background.
    let widget_bg = egui::Color32::from_rgb(30, 33, 42);
    let widget_bg_hovered = egui::Color32::from_rgb(40, 44, 55);
    let widget_bg_active = egui::Color32::from_rgb(50, 55, 68);

    visuals.widgets.inactive.weak_bg_fill = widget_bg;
    visuals.widgets.inactive.bg_fill = widget_bg;

    visuals.widgets.hovered.weak_bg_fill = widget_bg_hovered;
    visuals.widgets.hovered.bg_fill = widget_bg_hovered;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.active.weak_bg_fill = widget_bg_active;
    visuals.widgets.active.bg_fill = widget_bg_active;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, ACCENT);

    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(60));

    // Hyperlink colour.
    visuals.hyperlink_color = ACCENT;

    ctx.set_visuals(visuals);
}
