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
// Registry — W5 built-in templates.
// ---------------------------------------------------------------------------

/// Lazy-initialised registry of built-in scene templates.
///
/// Uses `std::sync::LazyLock` (stable since Rust 1.80) so the `String`
/// fields can be heap-allocated at first access without `unsafe`. The
/// registry is read-only after initialisation.
static SCENE_REGISTRY: std::sync::LazyLock<Vec<SceneTemplate>> = std::sync::LazyLock::new(|| {
    vec![
        // P4.5.1 — window_reveal
        SceneTemplate {
            id: "window_reveal".to_string(),
            display_name: "Window Reveal".to_string(),
            description: "A soft reveal that flows light through tagged window zones.".to_string(),
            zones_consumed: vec![ZoneRole::Window],
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
        },
        // P4.5.2 — pixel_drift
        SceneTemplate {
            id: "pixel_drift".to_string(),
            display_name: "Pixel Drift".to_string(),
            description: "Fine particles drift gently across the source media.".to_string(),
            zones_consumed: vec![],
            media_slots: vec![MediaSlotDescriptor {
                name: "source".to_string(),
                label: "Source media".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_constrained_drift".to_string()],
            palette: PaletteHint::Cool,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.3 — collage_bloom
        SceneTemplate {
            id: "collage_bloom".to_string(),
            display_name: "Collage Bloom".to_string(),
            description:
                "A four-image collage with particles blooming from the edges of each image."
                    .to_string(),
            zones_consumed: vec![],
            media_slots: vec![
                MediaSlotDescriptor {
                    name: "slot_a".to_string(),
                    label: "Image A".to_string(),
                    accepts: vec![MediaSlotKind::Image],
                },
                MediaSlotDescriptor {
                    name: "slot_b".to_string(),
                    label: "Image B".to_string(),
                    accepts: vec![MediaSlotKind::Image],
                },
                MediaSlotDescriptor {
                    name: "slot_c".to_string(),
                    label: "Image C".to_string(),
                    accepts: vec![MediaSlotKind::Image],
                },
                MediaSlotDescriptor {
                    name: "slot_d".to_string(),
                    label: "Image D".to_string(),
                    accepts: vec![MediaSlotKind::Image],
                },
            ],
            fx_presets_used: vec!["mask_edge_emission".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Energetic,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.4 — glow_behind_openings
        SceneTemplate {
            id: "glow_behind_openings".to_string(),
            display_name: "Glow Behind Openings".to_string(),
            description: "Fluid light pools in portal zones, evoking glow from behind \
                              architectural openings."
                .to_string(),
            zones_consumed: vec![ZoneRole::Portal],
            media_slots: vec![MediaSlotDescriptor {
                name: "glow_source".to_string(),
                label: "Glow source".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_bounded_fluid".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Ethereal,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.5 — fragmented_portrait
        SceneTemplate {
            id: "fragmented_portrait".to_string(),
            display_name: "Fragmented Portrait".to_string(),
            description: "A portrait broken into fragments by colliding particles at the \
                              mask boundary."
                .to_string(),
            zones_consumed: vec![],
            media_slots: vec![MediaSlotDescriptor {
                name: "portrait".to_string(),
                label: "Portrait image".to_string(),
                accepts: vec![MediaSlotKind::Image],
            }],
            fx_presets_used: vec!["mask_collision_reflection".to_string()],
            palette: PaletteHint::Neutral,
            mood: MoodHint::Energetic,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.6 — architectural_wash
        // The underlying FX preset (mask_edge_ripple_wash) is unchanged;
        // this scene template adds media + zone composition.
        SceneTemplate {
            id: "architectural_wash".to_string(),
            display_name: "Architectural Wash".to_string(),
            description: "A gentle wave wash that traces the edges of architectural \
                              surfaces tagged as edge zones. Upgrade of the v3 Architectural \
                              Wash preset."
                .to_string(),
            zones_consumed: vec![ZoneRole::Edge],
            media_slots: vec![MediaSlotDescriptor {
                name: "surface".to_string(),
                label: "Architectural surface".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Cool,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.7 — mask_edge_ripple_wash_scene
        SceneTemplate {
            id: "mask_edge_ripple_wash_scene".to_string(),
            display_name: "Mask-Edge Ripple Wash (Scene)".to_string(),
            description: "The classic mask-edge ripple wash as a standalone scene. \
                              No media required."
                .to_string(),
            zones_consumed: vec![],
            media_slots: vec![],
            fx_presets_used: vec!["mask_edge_ripple_wash".to_string()],
            palette: PaletteHint::Neutral,
            mood: MoodHint::Calm,
            tempo_sync: false,
            builtin: true,
        },
        // P4.5.8 — light_spill_from_windows
        SceneTemplate {
            id: "light_spill_from_windows".to_string(),
            display_name: "Light Spill from Windows".to_string(),
            description: "Light appears to spill outward from tagged window zones, \
                              as if an interior source is leaking through the aperture."
                .to_string(),
            zones_consumed: vec![ZoneRole::Window],
            media_slots: vec![MediaSlotDescriptor {
                name: "interior".to_string(),
                label: "Interior light source".to_string(),
                accepts: vec![MediaSlotKind::Image, MediaSlotKind::Video],
            }],
            fx_presets_used: vec!["mask_field_flow".to_string()],
            palette: PaletteHint::Warm,
            mood: MoodHint::Ethereal,
            tempo_sync: false,
            builtin: true,
        },
    ]
});

/// All registered built-in scene templates.
///
/// Returns a reference to the static registry populated at first call.
pub fn scene_registry() -> &'static [SceneTemplate] {
    &SCENE_REGISTRY
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

    // W5 — built-in template tests.

    /// P4.5.1–P4.5.8 — registry has exactly 8 built-in templates.
    #[test]
    fn scene_registry_has_eight_builtin_templates() {
        assert_eq!(
            scene_registry().len(),
            8,
            "expected 8 built-in templates, found {}",
            scene_registry().len()
        );
    }

    /// P4.5.1–P4.5.8 — all registry IDs are unique.
    #[test]
    fn scene_registry_ids_are_unique() {
        let ids: Vec<&str> = scene_registry().iter().map(|t| t.id.as_str()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(*id), "duplicate template id: {id}");
        }
    }

    /// P4.5.1 — window_reveal is registered and has expected fields.
    #[test]
    fn window_reveal_is_registered() {
        assert!(scene_is_registered("window_reveal"));
        let t = scene_registry()
            .iter()
            .find(|t| t.id == "window_reveal")
            .unwrap();
        assert_eq!(
            t.zones_consumed,
            vec![crate::project::schema::ZoneRole::Window]
        );
        assert_eq!(t.media_slots.len(), 1);
        assert_eq!(t.fx_presets_used, vec!["mask_edge_ripple_wash"]);
        assert!(t.builtin);
    }

    /// P4.5.2 — pixel_drift is registered with no zones.
    #[test]
    fn pixel_drift_is_registered() {
        assert!(scene_is_registered("pixel_drift"));
        let t = scene_registry()
            .iter()
            .find(|t| t.id == "pixel_drift")
            .unwrap();
        assert!(t.zones_consumed.is_empty());
        assert_eq!(t.fx_presets_used, vec!["mask_constrained_drift"]);
    }

    /// P4.5.3 — collage_bloom has four media slots.
    #[test]
    fn collage_bloom_has_four_media_slots() {
        assert!(scene_is_registered("collage_bloom"));
        let t = scene_registry()
            .iter()
            .find(|t| t.id == "collage_bloom")
            .unwrap();
        assert_eq!(t.media_slots.len(), 4);
    }

    /// P4.5.7 — mask_edge_ripple_wash_scene has no media slots.
    #[test]
    fn mask_edge_ripple_wash_scene_has_no_media_slots() {
        assert!(scene_is_registered("mask_edge_ripple_wash_scene"));
        let t = scene_registry()
            .iter()
            .find(|t| t.id == "mask_edge_ripple_wash_scene")
            .unwrap();
        assert!(t.media_slots.is_empty());
        assert!(t.zones_consumed.is_empty());
    }

    /// P4.5.1–P4.5.8 — every template has a non-empty display_name and description.
    #[test]
    fn all_builtin_templates_have_labels() {
        for t in scene_registry() {
            assert!(
                !t.display_name.is_empty(),
                "template {} has empty display_name",
                t.id
            );
            assert!(
                !t.description.is_empty(),
                "template {} has empty description",
                t.id
            );
        }
    }

    /// P4.5.1–P4.5.8 — scene_display_label returns Some for all registered IDs.
    #[test]
    fn scene_display_label_returns_some_for_all_registered() {
        for t in scene_registry() {
            assert!(
                scene_display_label(&t.id).is_some(),
                "scene_display_label returned None for registered id: {}",
                t.id
            );
        }
    }

    // -----------------------------------------------------------------------
    // P4.8.1 — Proptest round-trip: SceneTemplate serde + registry invariants
    // -----------------------------------------------------------------------

    use proptest::prelude::*;

    /// Arbitrary `PaletteHint` strategy.
    fn arb_palette() -> impl Strategy<Value = PaletteHint> {
        prop_oneof![
            Just(PaletteHint::Warm),
            Just(PaletteHint::Cool),
            Just(PaletteHint::Neutral),
        ]
    }

    /// Arbitrary `MoodHint` strategy.
    fn arb_mood() -> impl Strategy<Value = MoodHint> {
        prop_oneof![
            Just(MoodHint::Calm),
            Just(MoodHint::Energetic),
            Just(MoodHint::Ethereal),
        ]
    }

    /// Arbitrary `ZoneRole` strategy.
    fn arb_zone_role() -> impl Strategy<Value = crate::project::schema::ZoneRole> {
        use crate::project::schema::ZoneRole;
        prop_oneof![
            Just(ZoneRole::Window),
            Just(ZoneRole::Portal),
            Just(ZoneRole::Void),
            Just(ZoneRole::Spill),
            Just(ZoneRole::Edge),
            Just(ZoneRole::Highlight),
            Just(ZoneRole::LightSource),
        ]
    }

    /// Arbitrary `MediaSlotKind` strategy.
    fn arb_media_slot_kind() -> impl Strategy<Value = MediaSlotKind> {
        prop_oneof![
            Just(MediaSlotKind::Image),
            Just(MediaSlotKind::Video),
            Just(MediaSlotKind::Any),
        ]
    }

    /// Arbitrary `MediaSlotDescriptor` strategy.
    fn arb_media_slot_descriptor() -> impl Strategy<Value = MediaSlotDescriptor> {
        (
            "[a-z]{1,10}",
            "[A-Za-z ]{1,20}",
            prop::collection::vec(arb_media_slot_kind(), 0..3),
        )
            .prop_map(|(name, label, accepts)| MediaSlotDescriptor {
                name,
                label,
                accepts,
            })
    }

    /// Arbitrary `SceneTemplate` strategy.
    fn arb_scene_template() -> impl Strategy<Value = SceneTemplate> {
        (
            "[a-z_]{1,20}",
            "[A-Za-z ]{1,20}",
            "[A-Za-z .]{1,50}",
            prop::collection::vec(arb_zone_role(), 0..4),
            prop::collection::vec(arb_media_slot_descriptor(), 0..4),
            prop::collection::vec("[a-z_]{1,20}", 0..3),
            arb_palette(),
            arb_mood(),
            proptest::bool::ANY,
            proptest::bool::ANY,
        )
            .prop_map(
                |(
                    id,
                    display_name,
                    description,
                    zones_consumed,
                    media_slots,
                    fx_presets_used,
                    palette,
                    mood,
                    tempo_sync,
                    builtin,
                )| {
                    SceneTemplate {
                        id,
                        display_name,
                        description,
                        zones_consumed,
                        media_slots,
                        fx_presets_used,
                        palette,
                        mood,
                        tempo_sync,
                        builtin,
                    }
                },
            )
    }

    proptest! {
        /// P4.8.1 — arbitrary SceneTemplate values serialise and deserialise
        /// without loss.
        #[test]
        fn proptest_scene_template_serde_round_trip(template in arb_scene_template()) {
            let json = serde_json::to_string(&template)
                .expect("SceneTemplate must be serialisable");
            let back: SceneTemplate = serde_json::from_str(&json)
                .expect("SceneTemplate JSON must be deserialisable");

            prop_assert_eq!(&back.id, &template.id);
            prop_assert_eq!(&back.display_name, &template.display_name);
            prop_assert_eq!(&back.description, &template.description);
            prop_assert_eq!(&back.zones_consumed, &template.zones_consumed);
            prop_assert_eq!(back.media_slots.len(), template.media_slots.len());
            prop_assert_eq!(&back.fx_presets_used, &template.fx_presets_used);
            prop_assert_eq!(back.palette, template.palette);
            prop_assert_eq!(back.mood, template.mood);
            prop_assert_eq!(back.tempo_sync, template.tempo_sync);
            prop_assert_eq!(back.builtin, template.builtin);
        }
    }

    /// P4.8.1 — registry uniqueness: no duplicate IDs.
    #[test]
    fn scene_registry_no_duplicate_ids_proptest_guard() {
        let ids: Vec<&str> = scene_registry().iter().map(|t| t.id.as_str()).collect();
        let unique_count = ids.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(ids.len(), unique_count, "duplicate IDs in scene_registry()");
    }
}
