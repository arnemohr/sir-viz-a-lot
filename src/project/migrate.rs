//! Schema-version migration registry. Even when only v1 exists, the entry
//! point exists so v2 is a one-function add, not a refactor.

use serde_json::Value;

use super::ProjectError;
use super::schema::CURRENT_SCHEMA_VERSION;

pub fn migrate(mut value: Value) -> Result<Value, ProjectError> {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    match version {
        v if v == CURRENT_SCHEMA_VERSION => Ok(value),
        // v0 (no field) and v1 are bit-compatible with v2 — only difference is
        // additive `Effect::External` (T-M7-07) which old files don't use.
        // v2 → v3 needs structural migration: each layer's flat `svg_path`
        // field becomes nested under `kind: { Svg: { svg_path } }` (T-M8-01).
        0 | 1 | 2 => {
            if version <= 2 {
                migrate_v2_to_v3_layers(&mut value);
            }
            value["schema_version"] = serde_json::json!(CURRENT_SCHEMA_VERSION);
            Ok(value)
        }
        v => Err(ProjectError::UnsupportedVersion(v)),
    }
}

/// Synthesize `LayerKind::Svg { svg_path }` for every v2 layer, removing the
/// old top-level `svg_path` field. v0/v1 layers also flow through this path:
/// at v0/v1 the same flat field existed, so the migration is identical.
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
        let out = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn project_v1_migrate_to_current() {
        let v = serde_json::json!({"schema_version": 1});
        let out = migrate(v).expect("migrate");
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
        let out = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v3");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        match &p.layers[0].kind {
            crate::project::schema::LayerKind::Svg { svg_path } => {
                assert_eq!(svg_path.to_str().unwrap(), "/tmp/a.svg");
            }
            other => panic!("expected Svg kind, got {other:?}"),
        }
    }
}
