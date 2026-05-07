//! Project file: load, save, version migration.

pub mod migrate;
pub mod schema;

pub use schema::Project;

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("I/O error accessing {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid project JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("unsupported schema_version {0}")]
    UnsupportedVersion(u32),
}

impl Project {
    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let text = fs::read_to_string(path).map_err(|source| ProjectError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        let value = migrate::migrate(value)?;
        let mut project: Project = serde_json::from_value(value)?;
        if project.warps.is_empty() {
            project.warps.push(schema::default_warp_mesh());
        }
        Ok(project)
    }

    /// Pretty-printed JSON to `path`, via a same-directory temp file + rename.
    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let json = serde_json::to_string_pretty(self)?;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(dir).map_err(|source| ProjectError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project.rmap.json");
        let tmp_path = dir.join(format!(".{fname}.{}.tmp", std::process::id()));
        fs::write(&tmp_path, json.as_bytes()).map_err(|source| ProjectError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        // On Unix `rename` atomically replaces an existing file. v1 targets macOS only;
        // a future Windows port will need MoveFileExW with MOVEFILE_REPLACE_EXISTING.
        fs::rename(&tmp_path, path).map_err(|source| {
            let _ = fs::remove_file(&tmp_path);
            ProjectError::Io {
                path: path.to_path_buf(),
                source,
            }
        })?;
        Ok(())
    }

    /// Resolve a path relative to the project file or to [`Project::asset_root`].
    #[allow(dead_code)] // T-M6: used when multi-asset-root project loading lands
    pub fn resolve_asset(&self, project_path: &Path, rel: &Path) -> PathBuf {
        let base = self
            .asset_root
            .as_deref()
            .unwrap_or_else(|| project_path.parent().unwrap_or(Path::new(".")));
        base.join(rel)
    }
}

/// Serialize the full project for scene slots (lossless via serde_json).
pub fn snapshot(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).expect("Project serializes to JSON")
}

/// Replace `project` from a snapshot produced by [`snapshot`].
pub fn restore(project: &mut Project, snap: &serde_json::Value) -> Result<(), serde_json::Error> {
    let p: Project = serde_json::from_value(snap.clone())?;
    *project = p;
    Ok(())
}

/// Linear-interpolate two snapshots field-by-field.
///
/// Numbers blend; objects recurse; equal-length arrays recurse element-wise.
/// Everything else (strings, booleans, mismatched-length arrays, null,
/// type-mismatched pairs) snaps at the midpoint — so a categorical change
/// like `BlendMode::Normal -> Add` flips at `t = 0.5`.
///
/// Used by the scene crossfade path (T-M7-04) when
/// [`Project::crossfade_duration_s`] is non-zero AND the two snapshots
/// share the same layer paths. Structural mismatches must be filtered
/// out by the caller — interpolating across them is well-defined here
/// (mid-point snap) but produces visible jolts that defeat the point of
/// a fade.
pub fn interpolate(
    a: &serde_json::Value,
    b: &serde_json::Value,
    t: f32,
) -> serde_json::Value {
    use serde_json::Value::{Array, Number, Object};
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Number(na), Number(nb)) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            let v = fa + (fb - fa) * t as f64;
            serde_json::Number::from_f64(v)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| if t < 0.5 { a.clone() } else { b.clone() })
        }
        (Array(aa), Array(bb)) if aa.len() == bb.len() => Array(
            aa.iter()
                .zip(bb.iter())
                .map(|(x, y)| interpolate(x, y, t))
                .collect(),
        ),
        (Object(ao), Object(bo)) => {
            let mut out = serde_json::Map::with_capacity(ao.len().max(bo.len()));
            for (k, va) in ao.iter() {
                match bo.get(k) {
                    Some(vb) => {
                        out.insert(k.clone(), interpolate(va, vb, t));
                    }
                    None if t < 0.5 => {
                        out.insert(k.clone(), va.clone());
                    }
                    None => {}
                }
            }
            for (k, vb) in bo.iter() {
                if !ao.contains_key(k) && t >= 0.5 {
                    out.insert(k.clone(), vb.clone());
                }
            }
            Object(out)
        }
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// True when both snapshots have a `layers` array of equal length and
/// matching `svg_path` per index. Used to gate crossfade scheduling: a
/// structural difference forces the recall to snap instantly so the
/// renderer's per-layer GPU state stays consistent with `project.layers`.
pub fn snapshots_share_layer_topology(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    let la = a.get("layers").and_then(|v| v.as_array());
    let lb = b.get("layers").and_then(|v| v.as_array());
    match (la, lb) {
        (Some(la), Some(lb)) if la.len() == lb.len() => la
            .iter()
            .zip(lb.iter())
            .all(|(x, y)| x.get("svg_path") == y.get("svg_path")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{BlendMode, LayerConfig, Scene, WarpMesh};
    use std::path::PathBuf;

    #[test]
    fn resolve_asset_default_to_project_dir() {
        let mut p = Project::default();
        p.asset_root = None;
        let proj = Path::new("shows/wedding/show.rmap.json");
        let got = p.resolve_asset(proj, Path::new("gfx/logo.svg"));
        assert_eq!(got, Path::new("shows/wedding/gfx/logo.svg"));
    }

    #[test]
    fn resolve_asset_honors_explicit_root() {
        let mut p = Project::default();
        p.asset_root = Some(PathBuf::from("assets/shared"));
        let proj = Path::new("shows/wedding/show.rmap.json");
        let got = p.resolve_asset(proj, Path::new("logo.svg"));
        assert_eq!(got, Path::new("assets/shared/logo.svg"));
    }

    #[test]
    fn project_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "rmap_round_trip_{}.rmap.json",
            std::process::id()
        ));

        let mut original = Project::default();
        original.output_monitor_index = 2;
        original.layers.push(LayerConfig {
            id: "layer_a".into(),
            svg_path: PathBuf::from("/tmp/fixture.svg"),
            enabled: true,
            transform: crate::project::schema::Transform2D::default(),
            effects: crate::effects::default_effect_chain(),
            blend_mode: BlendMode::Screen,
            opacity: 0.5,
        });
        original.warps.push(WarpMesh {
            rows: 1,
            cols: 1,
            grid: vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]],
            source_rect: [0.0, 0.0, 1.0, 1.0],
            mask_polygon: vec![],
            mask_feather: 0.05,
        });
        original.scenes.push(Scene {
            name: "intro".into(),
            snapshot: serde_json::json!({"k": 1}),
        });
        original.gamma = 1.8;
        original.background_color = [0.1, 0.2, 0.3, 1.0];

        original.save(&path).expect("save");
        let loaded = Project::load(&path).expect("load");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&loaded).unwrap()
        );
    }

    #[test]
    fn scene_snapshot_round_trip() {
        let mut p = Project::default();
        p.layers.push(LayerConfig {
            id: "a".into(),
            svg_path: PathBuf::from("fixtures/x.svg"),
            enabled: true,
            transform: crate::project::schema::Transform2D::default(),
            effects: crate::effects::default_effect_chain(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.75,
        });
        p.warps.push(WarpMesh {
            rows: 1,
            cols: 1,
            grid: vec![
                vec![[0.1, 0.0], [0.9, 0.05]],
                vec![[0.0, 1.0], [1.0, 1.0]],
            ],
            source_rect: [0.0, 0.0, 1.0, 1.0],
            mask_polygon: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8]],
            mask_feather: 0.1,
        });
        p.scenes.push(Scene {
            name: "slot1".into(),
            snapshot: serde_json::json!({}),
        });
        p.gamma = 2.2;
        p.brightness = 0.1;
        p.contrast = 1.1;

        let snap = snapshot(&p);
        let mut q = Project::default();
        restore(&mut q, &snap).expect("restore");

        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::to_value(&q).unwrap()
        );
    }

    #[test]
    fn interpolate_numbers_linearly() {
        let a = serde_json::json!({"x": 0.0});
        let b = serde_json::json!({"x": 10.0});
        let mid = interpolate(&a, &b, 0.5);
        let x = mid["x"].as_f64().expect("number");
        assert!((x - 5.0).abs() < 1e-6, "got {x}");
    }

    #[test]
    fn interpolate_strings_snap_at_midpoint() {
        let a = serde_json::json!("alpha");
        let b = serde_json::json!("beta");
        assert_eq!(interpolate(&a, &b, 0.4), a);
        assert_eq!(interpolate(&a, &b, 0.6), b);
    }

    #[test]
    fn interpolate_nested_objects() {
        let a = serde_json::json!({"o": {"x": 0.0, "name": "a"}});
        let b = serde_json::json!({"o": {"x": 10.0, "name": "b"}});
        let m = interpolate(&a, &b, 0.25);
        let x = m["o"]["x"].as_f64().expect("number");
        assert!((x - 2.5).abs() < 1e-6);
        assert_eq!(m["o"]["name"].as_str(), Some("a"));
    }

    #[test]
    fn interpolate_equal_length_arrays_recurse() {
        let a = serde_json::json!([0.0, 100.0]);
        let b = serde_json::json!([10.0, 200.0]);
        let m = interpolate(&a, &b, 0.5);
        assert!((m[0].as_f64().unwrap() - 5.0).abs() < 1e-6);
        assert!((m[1].as_f64().unwrap() - 150.0).abs() < 1e-6);
    }

    #[test]
    fn snapshots_topology_matches_when_paths_align() {
        let a = serde_json::json!({"layers": [{"svg_path": "/x.svg", "id": "a"}]});
        let b = serde_json::json!({"layers": [{"svg_path": "/x.svg", "id": "renamed"}]});
        assert!(snapshots_share_layer_topology(&a, &b));
    }

    #[test]
    fn snapshots_topology_diverges_on_path_change() {
        let a = serde_json::json!({"layers": [{"svg_path": "/x.svg"}]});
        let b = serde_json::json!({"layers": [{"svg_path": "/y.svg"}]});
        assert!(!snapshots_share_layer_topology(&a, &b));
    }
}
