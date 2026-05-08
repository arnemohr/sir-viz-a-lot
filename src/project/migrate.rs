//! Schema-version migration registry. Even when only v1 exists, the entry
//! point exists so v2 is a one-function add, not a refactor.

use serde_json::Value;

use super::ProjectError;
use super::schema::CURRENT_SCHEMA_VERSION;

/// Side-channel signal returned alongside the migrated value so callers
/// can populate `Project.transient_audit_signals` after deserialise.
/// Migration runs on `serde_json::Value` (before typed-deserialise) so
/// the side-channel can't be embedded in the typed struct itself.
#[derive(Debug, Default, Clone)]
pub struct MigrationOutcome {
    /// Number of `Project.warps` entries the v3 → v4 step consolidated
    /// onto per-layer warps. `0` for v4-native projects; `1` for the
    /// common case (no audit finding); `> 1` triggers the
    /// `MultipleWarpsConsolidated` finding (T3.0d).
    pub previous_warp_count: usize,
}

pub fn migrate(mut value: Value) -> Result<(Value, MigrationOutcome), ProjectError> {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let mut outcome = MigrationOutcome::default();

    match version {
        v if v == CURRENT_SCHEMA_VERSION => Ok((value, outcome)),
        // v0 (no field) and v1 are bit-compatible with v2 — only difference is
        // additive `Effect::External` (T-M7-07) which old files don't use.
        // v2 → v3 needs structural migration: each layer's flat `svg_path`
        // field becomes nested under `kind: { Svg: { svg_path } }` (T-M8-01).
        // v3 → v4 (T3.0a) copies the project-level `warps[0]` onto each
        // layer's new `warp` field. `Project.warps` is preserved during
        // T3.0a so the renderer + audit + mutations keep compiling; T3.0b
        // deletes it once the render graph reads per-layer warps.
        0 | 1 | 2 | 3 => {
            if version <= 2 {
                migrate_v2_to_v3_layers(&mut value);
            }
            if version <= 3 {
                migrate_v3_to_v4_per_layer_warp(&mut value, &mut outcome);
            }
            value["schema_version"] = serde_json::json!(CURRENT_SCHEMA_VERSION);
            Ok((value, outcome))
        }
        v => Err(ProjectError::UnsupportedVersion(v)),
    }
}

/// Synthesize `LayerKind::Svg { svg_path }` for every v2 layer, removing the
/// old top-level `svg_path` field. v0/v1 layers also flow through this path:
/// at v0/v1 the same flat field existed, so the migration is identical.
/// Copy `Project.warps[0]` (or an identity warp) onto each layer's new
/// `warp` field. Records the original warp count in `outcome` so the
/// audit pass (T3.0d) can fire `MultipleWarpsConsolidated` exactly once
/// when the migration was lossy (M > 1 warps consolidated to N layers).
///
/// The project's top-level `warps` array is **preserved**: the v4 render
/// graph hasn't landed yet (T3.0b), so the renderer, audit, and mutations
/// still read it. T3.0b deletes the field atomically with the render-graph
/// rewrite.
fn migrate_v3_to_v4_per_layer_warp(value: &mut Value, outcome: &mut MigrationOutcome) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    let template_warp: Value = obj
        .get("warps")
        .and_then(|w| w.as_array())
        .map(|warps| {
            outcome.previous_warp_count = warps.len();
            warps.first().cloned().unwrap_or_else(default_identity_warp)
        })
        .unwrap_or_else(default_identity_warp);

    let Some(layers) = obj.get_mut("layers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(layer_obj) = layer.as_object_mut() else {
            continue;
        };
        // Preserve a hand-edited per-layer warp if one is already present
        // (someone may have written a v4-shaped file by hand and tagged
        // it `schema_version: 3`).
        if !layer_obj.contains_key("warp") {
            layer_obj.insert("warp".into(), template_warp.clone());
        }
    }
}

/// JSON form of `WarpMesh::identity()` — kept inline so migrate doesn't
/// have to depend on serde-deriving the typed struct.
fn default_identity_warp() -> Value {
    serde_json::json!({
        "rows": 1,
        "cols": 1,
        "grid": [
            [[0.0, 0.0], [1.0, 0.0]],
            [[0.0, 1.0], [1.0, 1.0]]
        ],
        "source_rect": [0.0, 0.0, 1.0, 1.0],
        "mask_polygon": [],
        "mask_feather": 0.02,
    })
}

fn migrate_v2_to_v3_layers(value: &mut Value) {
    let Some(layers) = value.get_mut("layers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(obj) = layer.as_object_mut() else {
            continue;
        };
        // Already migrated (someone hand-edited a v3 file with `schema_version: 2`).
        if obj.contains_key("kind") {
            continue;
        }
        if let Some(svg_path) = obj.remove("svg_path") {
            obj.insert(
                "kind".into(),
                serde_json::json!({ "Svg": { "svg_path": svg_path } }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::migrate;
    use crate::project::Project;
    use crate::project::schema::CURRENT_SCHEMA_VERSION;

    #[test]
    fn project_v0_migrate() {
        let v = serde_json::json!({});
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn project_v1_migrate_to_current() {
        let v = serde_json::json!({"schema_version": 1});
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn project_unsupported_future_version_errors() {
        let v = serde_json::json!({"schema_version": 999});
        let err = migrate(v).unwrap_err();
        assert!(matches!(
            err,
            crate::project::ProjectError::UnsupportedVersion(999)
        ));
    }

    #[test]
    fn project_v2_migrate_synthesizes_layer_kind() {
        let v = serde_json::json!({
            "schema_version": 2,
            "layers": [
                {
                    "id": "a",
                    "svg_path": "/tmp/a.svg",
                    "enabled": true,
                    "transform": { "translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0] },
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0
                }
            ]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as current");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        match &p.layers[0].kind {
            crate::project::schema::LayerKind::Svg { svg_path } => {
                assert_eq!(svg_path.to_str().unwrap(), "/tmp/a.svg");
            }
            other => panic!("expected Svg kind, got {other:?}"),
        }
    }

    fn v3_layer(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "kind": { "Svg": { "svg_path": "/tmp/x.svg" } },
            "enabled": true,
            "transform": { "translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0] },
            "effects": [],
            "blend_mode": "Normal",
            "opacity": 1.0,
        })
    }

    /// T3.0a — v3 with a single warp: every layer ends up carrying a
    /// copy of `warps[0]`; outcome reports `previous_warp_count == 1`
    /// (no audit finding fires for the common case).
    #[test]
    fn migrate_v3_warps_copy_single_warp_to_each_layer() {
        let custom_warp = serde_json::json!({
            "rows": 2,
            "cols": 2,
            "grid": [
                [[0.0, 0.0], [0.5, 0.0], [1.0, 0.0]],
                [[0.0, 0.5], [0.5, 0.5], [1.0, 0.5]],
                [[0.0, 1.0], [0.5, 1.0], [1.0, 1.0]],
            ],
            "source_rect": [0.0, 0.0, 1.0, 1.0],
            "mask_polygon": [[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]],
            "mask_feather": 0.07,
        });
        let v = serde_json::json!({
            "schema_version": 3,
            "layers": [v3_layer("a"), v3_layer("b")],
            "warps": [custom_warp],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v4");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(outcome.previous_warp_count, 1);
        assert_eq!(p.layers.len(), 2);
        for layer in &p.layers {
            assert_eq!(layer.warp.rows, 2);
            assert_eq!(layer.warp.cols, 2);
            assert!((layer.warp.mask_feather - 0.07).abs() < 1e-6);
            assert_eq!(layer.warp.mask_polygon.len(), 4);
        }
    }

    /// T3.0a — v3 with multiple warps: `warps[0]` is consolidated onto
    /// every layer; outcome reports `previous_warp_count > 1` so the
    /// audit pass (T3.0d) can emit `MultipleWarpsConsolidated`.
    #[test]
    fn migrate_v3_warps_consolidate_per_layer_with_signal() {
        let v = serde_json::json!({
            "schema_version": 3,
            "layers": [v3_layer("a"), v3_layer("b"), v3_layer("c")],
            "warps": [
                serde_json::json!({
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "source_rect": [0.0, 0.0, 0.5, 1.0],
                    "mask_polygon": [], "mask_feather": 0.02,
                }),
                serde_json::json!({
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "source_rect": [0.5, 0.0, 0.5, 1.0],
                    "mask_polygon": [], "mask_feather": 0.02,
                }),
            ],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v4");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(outcome.previous_warp_count, 2);
        // Every layer received warps[0] (source_rect[0] == 0.0, not 0.5).
        for layer in &p.layers {
            assert_eq!(layer.warp.source_rect[0], 0.0);
        }
    }

    /// T3.0a — v3 with zero warps: each layer ends up with the identity
    /// warp; outcome reports `previous_warp_count == 0` (no audit
    /// finding).
    #[test]
    fn migrate_v3_warps_zero_warps_seeds_identity() {
        let v = serde_json::json!({
            "schema_version": 3,
            "layers": [v3_layer("a")],
            "warps": [],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v4");
        assert_eq!(outcome.previous_warp_count, 0);
        assert_eq!(p.layers[0].warp.rows, 1);
        assert_eq!(p.layers[0].warp.cols, 1);
        assert!(p.layers[0].warp.mask_polygon.is_empty());
    }

    /// T3.0a — v4-native projects pass through unchanged; outcome
    /// reports `previous_warp_count == 0` so the audit finding never
    /// fires for fresh projects.
    #[test]
    fn migrate_v4_native_passes_through() {
        let v = serde_json::json!({
            "schema_version": 4,
            "layers": [],
            "warps": [],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        assert_eq!(out["schema_version"], serde_json::json!(4));
        assert_eq!(outcome.previous_warp_count, 0);
    }
}
