//! Crate-wide error type. Modules return `Result<T, RmapError>`; `main`
//! converts to `anyhow::Result` for top-level context.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, RmapError>;

#[derive(Debug, Error)]
pub enum RmapError {
    #[error("project I/O error")]
    Project(#[from] crate::project::ProjectError),

    #[error("renderer error")]
    Render(#[from] crate::render::RenderError),

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
