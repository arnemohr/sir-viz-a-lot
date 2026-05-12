//! P4.2.1 — Scene template schema + registry skeleton.
//!
//! A `SceneTemplate` is a read-only recipe that assembles existing primitives
//! (FX presets from Phase 2, zones from Phase 3, media slots) into a
//! ready-to-run scene.  Templates are applied via the wizard; the resulting
//! layers live in `project.layers` as ordinary `LayerConfig` entries.
//!
//! **Template identity is NOT tracked on the live layer** — "which template
//! produced this layer" is not stored.  See `004-phase-4-tasks.md` Anticipated
//! risk #1.
//!
//! # Registry pattern
//!
//! Mirrors `src/render/fx_presets.rs`:
//! - `scene_registry()` returns a `&'static [SceneTemplate]` slice.
//! - `scene_is_registered(id)` / `scene_display_label(id)` are convenience
//!   free functions.
//! - Built-in templates are compiled into the binary (no on-disk distribution).
//! - User templates live at `~/Library/Application Support/rmap/scenes/` (IO
//!   handled by P4.2.2's `src/windows/scene_io.rs`).

use serde::{Deserialize, Serialize};

use crate::project::schema::ZoneRole;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// What kind of media a slot accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSlotKind {
    Image,
    Video,
    Any,
}

/// Describes a single named media input slot in a scene template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSlotDescriptor {
    /// Stable machine identifier (e.g. `"bg"`, `"portrait"`).
    pub name: String,
    /// Operator-facing label shown in the wizard media step.
    pub label: String,
    /// Which media types this slot accepts.
    pub accepts: Vec<MediaSlotKind>,
}

/// Default colour accent: warm (amber/gold), cool (blue/cyan), or neutral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteHint {
    Warm,
    Cool,
    Neutral,
}

/// Emotional character of the scene: calm, energetic, or ethereal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoodHint {
    Calm,
    Energetic,
    Ethereal,
}

/// A read-only recipe for a scene.
///
/// Templates are applied via the scene wizard; they contain no warp geometry
/// (which would be projector-specific and not portable).  Zones are addressed
/// by semantic role; FX presets are referenced by their registry ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTemplate {
    /// Stable machine identifier (e.g. `"window_reveal"`).
    pub id: String,
    /// Operator-facing label (e.g. `"Window Reveal"`).
    pub display_name: String,
    /// One-sentence operator-facing description.
    pub description: String,
    /// Zone roles this template binds to.  Empty = full-canvas (no zone
    /// binding required).
    pub zones_consumed: Vec<ZoneRole>,
    /// Named media slots the template accepts.
    pub media_slots: Vec<MediaSlotDescriptor>,
    /// FX preset IDs (from `src/render/fx_presets.rs`) this template
    /// activates.
    pub fx_presets_used: Vec<String>,
    /// Default palette hint (operator can override in the wizard).
    pub palette: PaletteHint,
    /// Default mood hint (operator can override in the wizard).
    pub mood: MoodHint,
    /// Whether the template ties animation speed to the project BPM.
    pub tempo_sync: bool,
    /// `true` for compiled-in templates; `false` for user-exported templates.
    /// Built-in templates are read-only (the save function returns an error).
    pub builtin: bool,
}

// ---------------------------------------------------------------------------
// Registry — empty for now; W5 tasks add entries.
// ---------------------------------------------------------------------------

/// All registered built-in scene templates.
///
/// Empty at P4.2.1 stage; each W5 task appends one entry.  The slice is
/// `'static` (mirrors `fx_registry()` in `src/render/fx_presets.rs`).
pub fn scene_registry() -> &'static [SceneTemplate] {
    // W5 tasks will replace this with a populated static slice.
    &[]
}

/// Returns `true` if `id` corresponds to a registered scene template.
///
/// Consumed by W3/W4 wizard step UIs and P4.2.4 audit pass.
#[allow(dead_code)] // wired by W3 wizard + P4.2.4 audit
pub fn scene_is_registered(id: &str) -> bool {
    scene_registry().iter().any(|t| t.id == id)
}

/// Returns the operator-facing display label for `id`, or `None` if not
/// registered.
///
/// Consumed by W3/W4 wizard step UIs.
#[allow(dead_code)] // wired by W3 wizard step UIs
pub fn scene_display_label(id: &str) -> Option<&'static str> {
    scene_registry()
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.display_name.as_str())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_registry_does_not_panic() {
        // Empty at P4.2.1; will grow as W5 templates land.
        let _ = scene_registry();
    }

    #[test]
    fn scene_is_registered_unknown_returns_false() {
        assert!(!scene_is_registered("nonexistent_template"));
    }

    #[test]
    fn scene_display_label_unknown_returns_none() {
        assert_eq!(scene_display_label("nonexistent_template"), None);
    }

    #[test]
    fn scene_template_serde_round_trip() {
        let template = SceneTemplate {
            id: "test_template".to_string(),
            display_name: "Test Template".to_string(),
            description: "A test template for round-trip verification.".to_string(),
            zones_consumed: vec![ZoneRole::Window, ZoneRole::Edge],
            media_slots: vec![MediaSlotDescriptor {
                name: "bg".to_string(),
                label: "Background image".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        };

        let json = serde_json::to_string(&template).expect("serialize");
        let back: SceneTemplate = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.id, template.id);
        assert_eq!(back.display_name, template.display_name);
        assert_eq!(back.zones_consumed, template.zones_consumed);
        assert_eq!(back.media_slots.len(), template.media_slots.len());
        assert_eq!(back.fx_presets_used, template.fx_presets_used);
        assert_eq!(back.palette, template.palette);
        assert_eq!(back.mood, template.mood);
        assert_eq!(back.tempo_sync, template.tempo_sync);
        assert_eq!(back.builtin, template.builtin);
    }

    #[test]
    fn media_slot_kind_round_trips() {
        for kind in [
            MediaSlotKind::Image,
            MediaSlotKind::Video,
            MediaSlotKind::Any,
        ] {
            let json = serde_json::to_string(&kind).expect("serialize");
            let back: MediaSlotKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn palette_hint_round_trips() {
        for hint in [PaletteHint::Warm, PaletteHint::Cool, PaletteHint::Neutral] {
            let json = serde_json::to_string(&hint).expect("serialize");
            let back: PaletteHint = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, hint);
        }
    }

    #[test]
    fn mood_hint_round_trips() {
        for hint in [MoodHint::Calm, MoodHint::Energetic, MoodHint::Ethereal] {
            let json = serde_json::to_string(&hint).expect("serialize");
            let back: MoodHint = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, hint);
        }
    }
}
