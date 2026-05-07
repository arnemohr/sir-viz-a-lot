//! Project file: load, save, version migration.

pub mod migrate;
pub mod schema;

pub use schema::Project;

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid project JSON: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("unsupported schema_version {0}")]
    UnsupportedVersion(u32),
}

impl Project {
    pub fn load(_path: &Path) -> Result<Self, ProjectError> {
        // TODO(M6): read file, run through migrate::migrate(value), deserialize.
        Ok(Self::default())
    }

    pub fn save(&self, _path: &Path) -> Result<(), ProjectError> {
        // TODO(M6): serialize, write atomically (tmp + rename).
        Ok(())
    }
}
