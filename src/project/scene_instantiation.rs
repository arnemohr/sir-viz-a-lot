//! P4.2.3 — Scene template instantiation via `ApplyProjectSnapshot`.
//!
//! The wizard commit path (P4.3.3) calls `instantiate_template` to build a
//! new project JSON, then dispatches `Mutation::ApplyProjectSnapshot { new:
//! generated_json, old: pre_wizard_snapshot, non_undoable: false }`.
//!
//! # Design
//!
//! `instantiate_template` is a **pure function** (no side effects, no undo
//! stack entries). It operates on a scratch clone of the base project JSON so
//! the main project is never touched until the wizard commits.
//!
//! Layers are built by constructing `LayerConfig` values directly and
//! serialising them into the project JSON. The scratch project is never
//! dispatched through the undo stack during instantiation.
//!
//! # Zone bindings
//!
//! When `choices.zone_bindings` contains `ZoneRole` entries, the
//! instantiation assigns the first bound role to the warp mesh of the
//! first FxLayer in the template's `fx_presets_used` list. If no zone
//! bindings are provided (Phase 3 not yet active or the operator skipped
//! the zone step), all layers are instantiated without a zone tag
//! (full-canvas fallback).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::project::scene_templates::{MoodHint, PaletteHint, SceneTemplate};
use crate::project::schema::{Project, ZoneRole, layer_from_fx_preset, layer_from_image_path};

// ---------------------------------------------------------------------------
// Wizard choices
// ---------------------------------------------------------------------------

/// Collected inputs from the scene wizard (all five steps).
///
/// Defined here because `instantiate_template` is the primary consumer.
/// The wizard module (P4.3.1) re-exports this type as `pub use`.
#[allow(dead_code)] // wired by W3 wizard (P4.3.1–P4.3.3) + W4 step UIs
#[derive(Debug, Clone, Default)]
pub struct WizardChoices {
    /// The selected template ID (from `scene_registry()`).
    pub template_id: String,
    /// Assigned media paths by slot name.  Unassigned slots produce layers
    /// with empty paths (the operator can assign media post-commit).
    pub media_slots: HashMap<String, PathBuf>,
    /// Zone roles bound in the wizard zone step.  Empty when Phase 3 zone
    /// tagging is not active or the operator skipped the step.
    pub zone_bindings: Vec<ZoneRole>,
    /// Palette override.  When `None`, the template default is used.
    pub palette: Option<PaletteHint>,
    /// Mood override.  When `None`, the template default is used.
    pub mood: Option<MoodHint>,
    /// Whether to lock animation to the project BPM.
    pub tempo_sync: bool,
}

// ---------------------------------------------------------------------------
// Instantiation
// ---------------------------------------------------------------------------

/// Build a project JSON from `template` + `choices`, starting from `base_project`.
///
/// The function:
/// 1. Clears all layers from the base project.
/// 2. Adds one `Image/Video` layer per media slot in the template.
/// 3. Adds one `FxLayer` per FX preset ID in `template.fx_presets_used`.
/// 4. Assigns `ZoneRole` to FxLayer warp meshes if `choices.zone_bindings`
///    is non-empty.
/// 5. Returns the resulting JSON without modifying the caller's state.
///
/// The returned `Value` can be passed directly to
/// `Mutation::ApplyProjectSnapshot { new: …, old: pre_wizard_snapshot }`.
///
/// # Panics
///
/// Never panics. Returns `base_project` unchanged if the template is not
/// found in the registry (defensive fallback).
#[allow(dead_code)] // wired by P4.3.3 wizard commit
pub fn instantiate_template(
    template: &SceneTemplate,
    choices: &WizardChoices,
    base_project: serde_json::Value,
) -> serde_json::Value {
    // Deserialise the base project; return unchanged on failure.
    let mut project: Project = match serde_json::from_value(base_project.clone()) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(
                "instantiate_template: cannot deserialise base_project: {e}; \
                 returning base project unchanged"
            );
            return base_project;
        }
    };

    // 1. Clear all existing layers.
    project.layers.clear();

    // 2. Add one Image layer per media slot.
    for (slot_idx, slot) in template.media_slots.iter().enumerate() {
        let path = choices
            .media_slots
            .get(&slot.name)
            .cloned()
            .unwrap_or_default();
        let id = format!("scene_{}_media_{slot_idx}", template.id);
        let layer = layer_from_image_path(id, path);
        project.layers.push(layer);
    }

    // 3. Add one FxLayer per preset ID.
    let mut zone_iter = choices.zone_bindings.iter().copied();
    for (fx_idx, preset_id) in template.fx_presets_used.iter().enumerate() {
        let id = format!("scene_{}_fx_{fx_idx}", template.id);
        let mut layer = layer_from_fx_preset(id, preset_id.clone(), HashMap::new(), 0);

        // 4. Assign zone role from bindings if available.
        if let Some(role) = zone_iter.next() {
            layer.warp.zone_role = Some(role);
        }

        project.layers.push(layer);
    }

    // 5. Serialise back to JSON.
    match serde_json::to_value(&project) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                "instantiate_template: cannot serialise resulting project: {e}; \
                 returning base project unchanged"
            );
            base_project
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::scene_templates::{MediaSlotDescriptor, MediaSlotKind};
    use crate::project::snapshot;

    fn minimal_template(id: &str) -> SceneTemplate {
        SceneTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            description: "Test template.".to_string(),
            zones_consumed: vec![],
            media_slots: vec![MediaSlotDescriptor {
                name: "slot_0".to_string(),
                label: "Media".to_string(),
                accepts: vec![MediaSlotKind::Image],
            }],
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        }
    }

    fn blank_project_json() -> serde_json::Value {
        let project = Project::default();
        snapshot(&project)
    }

    /// P4.2.3 — one media slot → one layer with the assigned path.
    #[test]
    fn instantiate_assigns_media_slot_path() {
        let template = minimal_template("test_assign");
        let mut media_slots = HashMap::new();
        media_slots.insert("slot_0".to_string(), PathBuf::from("/tmp/photo.jpg"));
        let choices = WizardChoices {
            template_id: "test_assign".to_string(),
            media_slots,
            ..Default::default()
        };

        let result = instantiate_template(&template, &choices, blank_project_json());
        let back: Project = serde_json::from_value(result).expect("deserialise result");

        // One image layer + one FxLayer = 2 layers total.
        assert_eq!(
            back.layers.len(),
            2,
            "expected 2 layers: 1 image + 1 FxLayer"
        );

        // The image layer path should match the choice.
        match &back.layers[0].kind {
            crate::project::schema::LayerKind::Image { path, .. } => {
                assert_eq!(path, &PathBuf::from("/tmp/photo.jpg"));
            }
            other => panic!("expected Image layer, got {other:?}"),
        }
    }

    /// P4.2.3 — empty media_slots map → layer with empty path, no panic.
    #[test]
    fn instantiate_empty_media_slots_no_panic() {
        let template = minimal_template("test_empty");
        let choices = WizardChoices::default();

        let result = instantiate_template(&template, &choices, blank_project_json());
        let back: Project = serde_json::from_value(result).expect("deserialise result");

        // Still produces the right number of layers.
        assert_eq!(
            back.layers.len(),
            2,
            "expected 2 layers even with no media assigned"
        );
    }

    /// P4.2.3 — returned JSON deserialises to a valid Project.
    #[test]
    fn instantiate_result_is_valid_project() {
        let template = minimal_template("test_valid");
        let choices = WizardChoices::default();
        let result = instantiate_template(&template, &choices, blank_project_json());
        let back: Project = serde_json::from_value(result).expect("result must be a valid Project");
        // schema_version is preserved from the base project.
        assert_eq!(
            back.schema_version,
            crate::project::schema::CURRENT_SCHEMA_VERSION
        );
    }

    /// P4.2.3 — FX-only template (no media slots) produces a one-layer project.
    #[test]
    fn instantiate_fx_only_template_one_layer() {
        let template = SceneTemplate {
            id: "fx_only".to_string(),
            display_name: "FX Only".to_string(),
            description: "No media.".to_string(),
            zones_consumed: vec![],
            media_slots: vec![], // no media slots
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Neutral,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        };
        let choices = WizardChoices::default();
        let result = instantiate_template(&template, &choices, blank_project_json());
        let back: Project = serde_json::from_value(result).expect("deserialise");
        assert_eq!(
            back.layers.len(),
            1,
            "FX-only template must produce one FxLayer"
        );
    }
}
