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
        // v0 (no field) and v1 are bit-compatible with v2: the only
        // change for v2 is the additive `Effect::External` enum variant
        // (T-M7-07), which a v0/v1 file by definition does not use. So
        // both old versions stamp straight to current.
        0 | 1 => {
            value["schema_version"] = serde_json::json!(CURRENT_SCHEMA_VERSION);
            Ok(value)
        }
        v => Err(ProjectError::UnsupportedVersion(v)),
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
}
