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
}
