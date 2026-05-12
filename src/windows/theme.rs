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

// ── Background hierarchy ────────────────────────────────────────────────────
//
// Three steps so the operator's eye reads the panel structure at a glance:
//   BG_DEEP    — main app background; far behind everything
//   BG_PANEL   — side panels, top bar, the dominant "chrome" surface
//   BG_RAISED  — inputs / buttons; lifted off the panel so they look hittable
//
// Cool neutral greys (warmer than pure-grey, cooler than slate) so the gold
// accent reads as the only warm note in the UI.

pub const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(10, 11, 14);
/// Backwards-compat alias for BG_DEEP (was the v0.4 name).
pub const BG_BACKGROUND: egui::Color32 = BG_DEEP;
pub const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(22, 24, 30);
/// Raised surfaces — buttons, inputs, combobox heads. Slightly lighter
/// than `BG_PANEL` so a button on a panel reads as separable without a
/// stroke. Hover / active states bump this further.
pub const BG_RAISED: egui::Color32 = egui::Color32::from_rgb(32, 35, 42);

// ── Text hierarchy ──────────────────────────────────────────────────────────
//
// Three steps so labels never compete with values:
//   TEXT_PRIMARY    — values + headlines; full contrast
//   TEXT_SECONDARY  — labels next to values; ~70% contrast
//   TEXT_TERTIARY   — hints, file paths, empty-state copy; ~50% contrast
//
// All three meet WCAG AA against BG_PANEL.

pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(228, 230, 235);
pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(160, 165, 175);
#[allow(dead_code)] // surfacing tertiary-tier hints lands incrementally
pub const TEXT_TERTIARY: egui::Color32 = egui::Color32::from_rgb(110, 115, 125);

/// Subtle dividing stroke between sections — cool grey at ~25% panel luminance.
#[allow(dead_code)] // ditto — incremental adoption as section dividers ship.
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(46, 50, 60);

// ── Accent ───────────────────────────────────────────────────────────────────

/// Warm gold — the primary interactive accent. Softened from the v0.4
/// (255, 200, 100) so it reads as a designer choice rather than a hot
/// highlight, while keeping plenty of contrast against the cool panel
/// backgrounds.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(235, 190, 110);
/// Dimmed warm gold — used for unselected handles and secondary accent hits.
pub const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(175, 140, 75);

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
/// colour overrides. Also installs rmap's spacing + typography overrides
/// — the v0.5 pass loosened item spacing and bumped button padding so
/// the control-window panels stop feeling cramped under the dense
/// Treatment / Effect-chain rows.
pub fn install(ctx: &egui::Context) {
    let mut style: egui::Style = (*ctx.global_style()).clone();

    // ── Visuals ────────────────────────────────────────────────────────
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.extreme_bg_color = BG_DEEP;

    // Override the text colour for non-interactive widgets (labels, etc.).
    visuals.override_text_color = Some(TEXT_PRIMARY);

    // Selection highlight — translucent accent.
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    // Widget fill colours layered above BG_PANEL: inactive lifts off the
    // panel surface; hover + active brighten further so the operator
    // gets unmistakable pointer feedback without colour-flashing.
    let widget_bg = BG_RAISED;
    let widget_bg_hovered = egui::Color32::from_rgb(44, 48, 58);
    let widget_bg_active = egui::Color32::from_rgb(56, 62, 76);

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
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, BORDER);

    // Slightly more rounded widget corners — 4 px (egui default is 2)
    // reads as modern without straying into "macOS-glassy" territory.
    let r4 = egui::CornerRadius::same(4);
    visuals.widgets.noninteractive.corner_radius = r4;
    visuals.widgets.inactive.corner_radius = r4;
    visuals.widgets.hovered.corner_radius = r4;
    visuals.widgets.active.corner_radius = r4;
    visuals.widgets.open.corner_radius = r4;
    visuals.window_corner_radius = egui::CornerRadius::same(6);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);

    // Hyperlink colour.
    visuals.hyperlink_color = ACCENT;

    style.visuals = visuals;

    // ── Spacing ────────────────────────────────────────────────────────
    // egui defaults pack rows tightly (item_spacing.y = 3) which makes
    // the dense Advanced / Treatment panels read as a wall of text. Two
    // changes have outsized impact:
    //
    //   item_spacing.y  3 →  6   (twice the breathing room between rows)
    //   button_padding (4,1) → (10,5)  (visible padding inside buttons)
    //
    // Indent stays at the egui default (18) so collapsing-header content
    // remains aligned with its peers. interact_size.y bumps from 18 → 22
    // so sliders and dropdowns have a comfortable click target.
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 22.0;
    // Slightly larger gaps around panel + menu chrome so floating
    // surfaces don't feel pinned to the edge.
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.menu_margin = egui::Margin::same(8);

    // ── Typography ─────────────────────────────────────────────────────
    // Bump heading + button up from egui defaults so section titles
    // (CollapsingHeader labels) and primary buttons read as distinct
    // tiers. Body stays at 14 — the dense slider / combobox rows are
    // already tightly packed; making them larger would force us to
    // re-think the row layouts.
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(17.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );

    ctx.set_global_style(style);
}
