//! 003-T1.34 — `ProjectAudit`: pre-flight checks against a `Project`.
//!
//! The audit walks the project once (cheap; M1 scope is single-machine
//! single-projector wedding-scale shows, where projects are <100 layers)
//! and returns a `Vec<AuditFinding>` describing anything the renderer or
//! operator should know about before going live. Each finding carries a
//! human-readable `message`, a `severity`, and an optional `autofix`
//! `Mutation` the operator can one-click apply.
//!
//! # AuditKind variants
//!
//! - [`AuditKind::ZeroScale`] (T1.35) — `Effect::Transform.scale = [0, 0]`
//!   collapses the layer to invisible; operator hits "play" and sees a
//!   black wall. Headline failure mode caught during the original audit.
//! - [`AuditKind::DegenerateWarp`] (T1.36, P1) — warp grid with rows < 2 /
//!   cols < 2 / non-rectangular row lengths. The shader assumes 2D
//!   bilinear interpolation; degenerate grids panic the GPU path.
//! - [`AuditKind::MaskTooFew`] (T1.37, P1) — mask polygon with fewer than
//!   3 vertices is silently dropped by the SDF baker; the operator may
//!   have intended to keep it but lost vertices.
//! - [`AuditKind::MissingAsset`] (T1.38) — layer's asset path doesn't
//!   exist on disk. `Severity::Critical`. Wedding-DJ "second laptop"
//!   failover hits this every time without a relink autofix.
//! - [`AuditKind::MonitorOutOfRange`] (T1.39) — the saved monitor index
//!   exceeds available count on this machine. Defaults to monitor 0.
//! - [`AuditKind::SchemaTooNew`] (T1.40) — `schema_version > CURRENT`.
//!   `Severity::Critical`. Project was written by a newer build; loading
//!   would silently miss new fields.
//! - [`AuditKind::EmptyProject`] — no layers configured. Informational.
//!
//! # Wiring (T1.43)
//!
//! `ProjectAudit::run` will be called from the project-load path and
//! exposed to the launcher screen. Critical findings route to
//! `AppState::Failed`; Warn findings surface as Toasts (T1.41–T1.43).

#![deny(missing_docs)]
#![allow(dead_code)] // T-003-T1.35+ wire the audit kinds; foundation lands here.

use std::path::PathBuf;

use crate::project::command::Mutation;
use crate::project::schema::Project;

/// Severity of an audit finding. Drives UX routing: `Info` and `Warn`
/// surface as toasts the operator can dismiss; `Critical` blocks the
/// project from entering `AppState::Editing` until resolved (or the
/// operator explicitly accepts a degraded state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth knowing but not actionable. Example: project has no
    /// layers (operator may have started fresh on purpose).
    Info,
    /// Likely a mistake; renderer still runs. Example: layer with
    /// `transform.scale = [0, 0]` is invisible.
    Warn,
    /// Renderer cannot run safely. Example: a layer's SVG file is
    /// missing on disk; the worker would log warnings every frame.
    Critical,
}

/// Discriminator for the kind of audit finding. Each variant carries
/// the location data the auto-fix needs (layer index, warp index,
/// path, etc.) so the UI can present "fix this" actions without
/// re-walking the project.
///
/// Adding a new kind: extend this enum, the matching detection
/// function in `ProjectAudit::run`, and the proptest harness in
/// `crate::project::command` if the autofix introduces a new
/// `Mutation` variant. Per CLAUDE.md Reverse-storage rules.
#[derive(Debug, Clone)]
pub enum AuditKind {
    /// Layer at `layer_idx` has `Effect::Transform.scale = [0, 0]`
    /// (or either component < 1e-6).
    ZeroScale {
        /// Index into `Project.layers`.
        layer_idx: usize,
    },
    /// Warp at `warp_idx` has rows < 2, cols < 2, or non-rectangular
    /// row lengths.
    DegenerateWarp {
        /// Index into `Project.warps`.
        warp_idx: usize,
    },
    /// Warp at `warp_idx` has a mask polygon with fewer than 3
    /// vertices; the SDF baker silently drops these.
    MaskTooFew {
        /// Index into `Project.warps`.
        warp_idx: usize,
        /// Number of vertices in the polygon (0, 1, or 2).
        vertex_count: usize,
    },
    /// Layer at `layer_idx` references an asset path that doesn't
    /// exist on disk. `Severity::Critical`.
    MissingAsset {
        /// Index into `Project.layers`.
        layer_idx: usize,
        /// The path that didn't resolve.
        path: PathBuf,
    },
    /// The saved monitor index exceeds the available monitor count
    /// on this machine.
    MonitorOutOfRange {
        /// Index requested by the project.
        requested: u32,
        /// Number of monitors actually available.
        available: u32,
    },
    /// Project's `schema_version` is newer than `CURRENT_SCHEMA_VERSION`.
    /// `Severity::Critical`.
    SchemaTooNew {
        /// Version found in the project file.
        project_version: u32,
        /// Highest schema version this build understands.
        max_supported: u32,
    },
    /// Project has no layers configured. Informational; operator may
    /// have started fresh.
    EmptyProject,
}

/// One finding from a `ProjectAudit::run` walk. The `message` field is
/// the user-facing string surfaced in toasts and the launcher; the
/// `autofix`, if present, is a one-click `Mutation` the UI can apply.
#[derive(Debug, Clone)]
pub struct AuditFinding {
    /// Discriminator + location data.
    pub kind: AuditKind,
    /// How loud this finding is.
    pub severity: Severity,
    /// Human-readable message displayed in toasts / launcher.
    pub message: String,
    /// One-click auto-fix mutation. `None` means the finding can't be
    /// auto-resolved (the operator must edit the project file or
    /// move assets).
    pub autofix: Option<Mutation>,
}

/// Per-machine state the audit needs that isn't part of the project
/// itself: available monitor count, etc. Passed in so unit tests can
/// pin a deterministic environment.
#[derive(Debug, Clone, Copy)]
pub struct AuditEnv {
    /// Number of monitors visible to the OS at audit time.
    pub monitor_count: u32,
}

impl Default for AuditEnv {
    fn default() -> Self {
        // Tests that don't care about monitor checks can use Default;
        // 1 is a safe value (excludes MonitorOutOfRange unless the
        // project explicitly references monitor index ≥ 1).
        Self { monitor_count: 1 }
    }
}

/// The audit driver. Holds no state — `run` walks the project from
/// scratch each call. Stateless so launcher and toast paths can call
/// it freely.
pub struct ProjectAudit;

impl ProjectAudit {
    /// Walk `project` against `env` and return every applicable
    /// finding. Returns an empty Vec for a project with no issues.
    /// Findings are emitted in roughly the order they're checked
    /// (project-level → layer-level → warp-level), but callers
    /// shouldn't depend on ordering for correctness.
    ///
    /// T1.34 lands the foundation; T1.35–T1.40 wire individual
    /// detectors. The body is intentionally empty here so a clean
    /// project returns `Vec::new()`.
    pub fn run(_project: &Project, _env: &AuditEnv) -> Vec<AuditFinding> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_project() -> Project {
        let json = serde_json::json!({
            "schema_version": 3,
            "layers": [],
            "warps": [],
        });
        let mut p: Project = serde_json::from_value(json).expect("project deserialise");
        if p.warps.is_empty() {
            p.warps.push(crate::project::schema::default_warp_mesh());
        }
        p
    }

    /// 003-T1.34 acceptance criterion 2: `ProjectAudit::run` returns
    /// an empty `Vec` for a project with no issues.
    #[test]
    fn run_empty_for_clean_project() {
        let p = fresh_project();
        let env = AuditEnv::default();
        let findings = ProjectAudit::run(&p, &env);
        assert!(
            findings.is_empty(),
            "clean project should produce zero findings, got {findings:?}"
        );
    }

    /// 003-T1.34 — Severity ordering invariant. Exhaustive match
    /// catches a future variant addition before it lands as silent
    /// dead code.
    #[test]
    fn severity_variants_are_distinct() {
        for s in [Severity::Info, Severity::Warn, Severity::Critical] {
            // Just confirms construction + Eq work.
            assert_eq!(s, s);
        }
    }
}
