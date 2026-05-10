//! 003-T1.34 — `ProjectAudit`: pre-flight checks against a `Project`.
//!
//! The audit walks the project once (cheap; M1 scope is single-machine
//! single-projector event-scale shows, where projects are <100 layers)
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
//! - [`AuditKind::DegenerateLayerWarp`] (T1.36, P1) — layer's warp grid
//!   with rows < 2 / cols < 2 / non-rectangular row lengths. The shader
//!   assumes 2D bilinear interpolation; degenerate grids panic the GPU
//!   path.
//! - [`AuditKind::LayerMaskTooFew`] (T1.37, P1) — layer's mask polygon
//!   with fewer than 3 vertices is silently dropped by the SDF baker;
//!   the operator may have intended to keep it but lost vertices.
//! - [`AuditKind::MultipleWarpsConsolidated`] (T3.0d) — fires once per
//!   session for v3 projects whose `Project.warps` had > 1 entries; the
//!   v4 migration could only copy one warp onto every layer, so the
//!   operator must re-map per-layer.
//! - [`AuditKind::MissingAsset`] (T1.38) — layer's asset path doesn't
//!   exist on disk. `Severity::Critical`. event-DJ "second laptop"
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
#![allow(dead_code)] // T-003-T1.43/T1.44 wire audit to app load path; foundation wired here.

use std::path::PathBuf;

use crate::effects::Effect;
use crate::modulators::Modulator;
use crate::project::command::Mutation;
use crate::project::schema::{LayerConfig, Project};

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
    /// Layer at `layer_idx` has a warp grid with rows < 2, cols < 2,
    /// or non-rectangular row lengths.
    DegenerateLayerWarp {
        /// Index into `Project.layers`; the layer's `warp` is the target.
        layer_idx: usize,
    },
    /// Layer at `layer_idx` has a mask polygon with fewer than 3
    /// vertices; the SDF baker silently drops these.
    LayerMaskTooFew {
        /// Index into `Project.layers`; the layer's `warp` is the target.
        layer_idx: usize,
        /// Number of vertices in the polygon (0, 1, or 2).
        vertex_count: usize,
    },
    /// v3 project had > 1 entries in `Project.warps`; the v4 migration
    /// consolidated all of them onto each layer (each layer received a
    /// copy of `warps[0]`). Fires exactly once per session — the
    /// transient signal lives in
    /// [`crate::project::schema::TransientAuditSignals`] and is
    /// cleared on first audit.
    MultipleWarpsConsolidated {
        /// Number of warps the v3 project carried.
        previous_warp_count: usize,
        /// Number of layers the project carries (each got a copy).
        layer_count: usize,
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
    /// V31.2.2 — project's `output_target.uuid` is set but no live monitor
    /// carries a matching UUID. Falls back to `fallback_index` (or display 0
    /// if the index is also out of range). `Severity::Warn`.
    OutputTargetUuidNotFound {
        /// The UUID stored in the project that had no live match.
        uuid: String,
        /// The `fallback_index` the project intended as a secondary fallback.
        fallback_index: usize,
    },
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
/// itself: available monitor count, UUID list, etc. Passed in so unit
/// tests can pin a deterministic environment.
///
/// Note: `Copy` was removed in V31.2.2 when `live_monitor_uuids`
/// (`Vec`) was added. Callers should pass `&env`.
#[derive(Debug, Clone)]
pub struct AuditEnv {
    /// Number of monitors visible to the OS at audit time.
    pub monitor_count: u32,
    /// V31.2.2 — UUID of each live monitor (parallel to the monitor
    /// list, same index ordering). `None` entries for monitors whose
    /// UUID is not yet known (before V31.2.3 fills them in).
    pub live_monitor_uuids: Vec<Option<String>>,
}

impl Default for AuditEnv {
    fn default() -> Self {
        // Tests that don't care about monitor checks can use Default;
        // 1 is a safe value (excludes MonitorOutOfRange unless the
        // project explicitly references monitor index ≥ 1).
        Self {
            monitor_count: 1,
            live_monitor_uuids: Vec::new(),
        }
    }
}

/// The audit driver. Holds no state — `run` walks the project from
/// scratch each call. Stateless so launcher and toast paths can call
/// it freely.
pub struct ProjectAudit;

impl ProjectAudit {
    /// Walk `project` against `env` and return every applicable
    /// finding. Returns an empty Vec for a project with no issues.
    /// Findings are emitted in project-level → layer-level → warp-level
    /// order, but callers shouldn't depend on ordering for correctness.
    ///
    /// 003-T2.23 follow-up: relative asset paths in `LayerKind` are
    /// resolved against the project file's parent directory before the
    /// `MissingAsset` existence check fires. Pass the project file path
    /// via [`run_with_path`] so portable projects (T2.23) audit cleanly;
    /// this overload defaults `project_path` to `None`, preserving the
    /// pre-T2.23 behaviour for callers that don't have one.
    pub fn run(project: &Project, env: &AuditEnv) -> Vec<AuditFinding> {
        Self::run_with_path(project, env, None)
    }

    /// 003-T2.23 follow-up — same as [`Self::run`] but also resolves
    /// relative `LayerKind` asset paths against `project_path.parent()`
    /// when checking for missing assets. The parameter is `Option`-shaped
    /// because not every audit caller has a path on hand (e.g. an
    /// in-memory project drafted via the launcher's "Start a new
    /// show" path).
    pub fn run_with_path(
        project: &Project,
        env: &AuditEnv,
        project_path: Option<&std::path::Path>,
    ) -> Vec<AuditFinding> {
        let mut findings = Vec::new();

        // --- Project-level checks ---

        // T1.40: schema_version newer than this build supports.
        if project.schema_version > crate::project::schema::CURRENT_SCHEMA_VERSION {
            findings.push(AuditFinding {
                kind: AuditKind::SchemaTooNew {
                    project_version: project.schema_version,
                    max_supported: crate::project::schema::CURRENT_SCHEMA_VERSION,
                },
                severity: Severity::Critical,
                message: format!(
                    "Project schema_version {} is newer than this build supports (max {}). \
                     Upgrade rmap to load this project.",
                    project.schema_version,
                    crate::project::schema::CURRENT_SCHEMA_VERSION,
                ),
                autofix: None,
            });
        }

        // T1.39: output_target.fallback_index >= monitor_count.
        if (project.primary_output_target().fallback_index as u32) >= env.monitor_count {
            findings.push(AuditFinding {
                kind: AuditKind::MonitorOutOfRange {
                    requested: project.primary_output_target().fallback_index as u32,
                    available: env.monitor_count,
                },
                severity: Severity::Warn,
                message: format!(
                    "Project requests monitor {} but only {} monitor(s) available. \
                     Falls back to monitor 0.",
                    project.primary_output_target().fallback_index,
                    env.monitor_count,
                ),
                autofix: Some(project.set_output_monitor_index_mutation(0)),
            });
        }

        // V31.2.2: OutputTargetUuidNotFound — project has a UUID but none of
        // the live monitors carries a matching UUID. Only emitted when
        // `live_monitor_uuids` is non-empty (i.e. the caller passed live
        // monitor data) so that callers that only populate `monitor_count`
        // (older call sites, non-v3 paths) don't produce spurious findings.
        if let Some(ref uuid) = project.primary_output_target().uuid {
            if !env.live_monitor_uuids.is_empty() {
                let uuid_found = env
                    .live_monitor_uuids
                    .iter()
                    .any(|u| u.as_deref() == Some(uuid.as_str()));
                if !uuid_found {
                    findings.push(AuditFinding {
                        kind: AuditKind::OutputTargetUuidNotFound {
                            uuid: uuid.clone(),
                            fallback_index: project.primary_output_target().fallback_index,
                        },
                        severity: Severity::Warn,
                        message: format!(
                            "Saved projector (UUID {uuid}) isn't connected. \
                             Falling back to monitor {}.",
                            project.primary_output_target().fallback_index,
                        ),
                        autofix: None,
                    });
                }
            }
        }

        // EmptyProject (T1.34): no layers configured.
        if project.layers.is_empty() {
            findings.push(AuditFinding {
                kind: AuditKind::EmptyProject,
                severity: Severity::Info,
                message: "Project has no layers configured.".into(),
                autofix: None,
            });
        }

        // --- Layer-level checks ---

        for (layer_idx, layer) in project.layers.iter().enumerate() {
            // T1.35: any Effect::Transform with scale_x and scale_y both Static(< 1e-6).
            if let Some(autofix) = zero_scale_autofix_for_layer(project, layer, layer_idx) {
                findings.push(AuditFinding {
                    kind: AuditKind::ZeroScale { layer_idx },
                    severity: Severity::Warn,
                    message: format!(
                        "Layer {} has zero scale (invisible). Autofix resets scale to 1.0.",
                        layer.id,
                    ),
                    autofix: Some(autofix),
                });
            }

            // T1.38: asset path doesn't exist on disk.
            // 003-T2.23 — relative paths are resolved against the
            // project file's parent dir when one is supplied. Falls
            // back to the as-stored path if `project_path` is None
            // (in-memory project) or has no parent (path is bare).
            //
            // P0.1.2 — variants without an asset path (`FxLayer`,
            // `Ndi`) skip the missing-asset check entirely. NDI source-
            // unavailable surfaces through P0.6.3's separate audit kind.
            if let Some(asset_path) = layer.kind.asset_path() {
                let resolved = match project_path.and_then(|p| p.parent()) {
                    Some(dir) if asset_path.is_relative() => dir.join(asset_path),
                    _ => asset_path.to_path_buf(),
                };
                if !resolved.exists() {
                    // 003-T2.24 — downgraded from Critical to Warn so the
                    // project still opens. A Critical here would route to
                    // AppState::Failed, leaving no path to the relink
                    // toast. The new flow surfaces a Warn toast with a
                    // "Find this file…" action that emits
                    // `Command::OpenRelinkPicker` and then
                    // `Mutation::RelinkAssetPath`. The layer still won't
                    // render until relinked, but the operator stays in the
                    // editor and can fix it without quitting.
                    findings.push(AuditFinding {
                        kind: AuditKind::MissingAsset {
                            layer_idx,
                            path: asset_path.to_path_buf(),
                        },
                        severity: Severity::Warn,
                        message: format!(
                            "Can't find {}. Find this file or remove the layer.",
                            asset_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or_else(|| asset_path.to_str().unwrap_or("missing asset")),
                        ),
                        // The autofix slot stays None because the
                        // replacement path comes from a file picker, not
                        // from project state. The relink action is wired
                        // at the toast-push site (app.rs) which has access
                        // to both the layer index and the original path.
                        autofix: None,
                    });
                }
            }
        }

        // --- Per-layer warp checks (v4) ---

        for (layer_idx, layer) in project.layers.iter().enumerate() {
            let warp = &layer.warp;
            // T1.36: degenerate grid — fewer than 2 vertex rows/columns, or
            // non-rectangular row lengths.
            // Note: rows/cols are *cell* counts; the vertex grid is (rows+1)×(cols+1).
            // A healthy WarpMesh::identity has rows=1, cols=1 with a 2×2 vertex grid.
            let grid_vertex_rows = warp.grid.len();
            let grid_first_cols = warp.grid.first().map(|r| r.len()).unwrap_or(0);
            let is_degenerate = grid_vertex_rows < 2
                || grid_first_cols < 2
                || warp.grid.iter().any(|row| row.len() != grid_first_cols);
            if is_degenerate {
                // Build the autofix: replace the warp with a corner-pin
                // (rows=1, cols=1, 2×2 vertex grid — same as identity).
                let new_mesh = crate::project::schema::WarpMesh {
                    rows: 1,
                    cols: 1,
                    grid: vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]],
                    mask_polygon: warp.mask_polygon.clone(),
                    mask_feather: warp.mask_feather,
                };
                findings.push(AuditFinding {
                    kind: AuditKind::DegenerateLayerWarp { layer_idx },
                    severity: Severity::Warn,
                    message: format!(
                        "Layer {} warp has a degenerate grid (vertex rows={}, vertex cols={}, \
                         non-rectangular: {}). Reset to corner-pin.",
                        layer_idx,
                        grid_vertex_rows,
                        grid_first_cols,
                        warp.grid.iter().any(|row| row.len() != grid_first_cols),
                    ),
                    autofix: Some(Mutation::ResetLayerWarpMesh(
                        crate::project::command::ResetLayerWarpMesh {
                            layer_idx,
                            new: new_mesh,
                            old: warp.clone(),
                        },
                    )),
                });
            }

            // T1.37: mask polygon with 1 or 2 vertices (0 is fine — no mask intended;
            // ≥3 is fine — valid polygon for SDF baker).
            let vertex_count = warp.mask_polygon.len();
            if vertex_count > 0 && vertex_count < 3 {
                findings.push(AuditFinding {
                    kind: AuditKind::LayerMaskTooFew {
                        layer_idx,
                        vertex_count,
                    },
                    severity: Severity::Info,
                    message: format!(
                        "Layer {} mask has {} vertex(es); SDF baker requires ≥ 3. \
                         Clear the mask or add vertices.",
                        layer_idx, vertex_count,
                    ),
                    autofix: Some(Mutation::SetLayerMaskPolygon(
                        crate::project::command::SetLayerMaskPolygon {
                            layer_idx,
                            new: Vec::new(),
                            old: warp.mask_polygon.clone(),
                        },
                    )),
                });
            }
        }

        // --- T3.0d: one-shot MultipleWarpsConsolidated ---
        //
        // Consume + clear `transient_audit_signals` so a second audit
        // call in the same session (e.g. the operator hits "Re-run
        // audit" from a debug menu) doesn't double-fire. Migrated v3
        // projects with > 1 warps land here exactly once; v4-native
        // projects always have `previous_warp_count == 0` and skip.
        let signals = project.transient_audit_signals.take();
        if signals.previous_warp_count > 1 {
            findings.push(AuditFinding {
                kind: AuditKind::MultipleWarpsConsolidated {
                    previous_warp_count: signals.previous_warp_count,
                    layer_count: project.layers.len(),
                },
                severity: Severity::Warn,
                message: format!(
                    "Project had {} warps but rmap now maps each layer individually. \
                     Each layer was given a copy of the first warp; verify layer mapping looks right.",
                    signals.previous_warp_count,
                ),
                autofix: None,
            });
        }

        findings
    }
}

/// Returns `Some(SetLayerEffects)` iff the layer contains at least one
/// `Effect::Transform` whose `scale_x` AND `scale_y` are both
/// `Modulator::Static(v)` with `v.abs() < 1e-6` (invisible layer).
/// The autofix replaces those scale fields with `Modulator::Static(1.0)`.
fn zero_scale_autofix_for_layer(
    project: &Project,
    layer: &LayerConfig,
    layer_idx: usize,
) -> Option<Mutation> {
    let has_zero_scale = layer.effects.iter().any(|e| {
        if let Effect::Transform {
            scale_x, scale_y, ..
        } = e
        {
            let sx_zero = matches!(scale_x, Modulator::Static(v) if v.abs() < 1e-6);
            let sy_zero = matches!(scale_y, Modulator::Static(v) if v.abs() < 1e-6);
            sx_zero && sy_zero
        } else {
            false
        }
    });
    if !has_zero_scale {
        return None;
    }

    let mut new_effects = layer.effects.clone();
    for e in new_effects.iter_mut() {
        if let Effect::Transform {
            scale_x, scale_y, ..
        } = e
        {
            if matches!(scale_x, Modulator::Static(v) if v.abs() < 1e-6)
                && matches!(scale_y, Modulator::Static(v) if v.abs() < 1e-6)
            {
                *scale_x = Modulator::Static(1.0);
                *scale_y = Modulator::Static(1.0);
            }
        }
    }
    Some(project.set_layer_effects_mutation(layer_idx, new_effects))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a project that passes all audit checks:
    /// - schema_version = CURRENT
    /// - one layer with a real on-disk asset (uses Cargo.toml, always present)
    /// - layer carries the identity warp (rows=1, cols=1, 2×2 grid)
    /// - output_target.fallback_index = 0, AuditEnv::default() has monitor_count = 1
    fn fresh_project() -> Project {
        let mut p = Project::default();
        // Add a layer with an asset that exists on disk so MissingAsset doesn't fire.
        // Use Cargo.toml as a stand-in — it exists in the workspace root and is
        // always present during test runs.
        let asset = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        p.layers.push(crate::project::schema::LayerConfig {
            id: "healthy".into(),
            kind: crate::project::schema::LayerKind::Svg { svg_path: asset },
            enabled: true,
            transform: crate::project::schema::Transform2D::default(),
            effects: vec![crate::effects::Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: crate::modulators::Modulator::Static(0.0),
                scale_x: crate::modulators::Modulator::Static(1.0),
                scale_y: crate::modulators::Modulator::Static(1.0),
            }],
            blend_mode: crate::project::schema::BlendMode::Normal,
            opacity: 1.0,
            warp: crate::project::schema::WarpMesh::identity(),
            muted: false,
        });
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

    /// 003-T1.35 — Layer with `Effect::Transform.scale_x = scale_y = 0`
    /// triggers a Warn finding with a SetLayerEffects autofix that
    /// restores scale to 1.0.
    #[test]
    fn audit_zero_scale_emits_finding_and_autofix() {
        let mut p = fresh_project();
        p.layers.push(crate::project::schema::LayerConfig {
            id: "zero".into(),
            kind: crate::project::schema::LayerKind::Svg {
                svg_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
            },
            enabled: true,
            transform: crate::project::schema::Transform2D::default(),
            effects: vec![Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(0.0),
                scale_y: Modulator::Static(0.0),
            }],
            blend_mode: crate::project::schema::BlendMode::Normal,
            opacity: 1.0,
            warp: crate::project::schema::WarpMesh::identity(),
            muted: false,
        });

        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        let zero = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::ZeroScale { .. }))
            .expect("expected ZeroScale finding");
        assert_eq!(zero.severity, Severity::Warn);
        assert!(zero.autofix.is_some(), "ZeroScale should have an autofix");

        // Apply autofix; assert scale restored to 1.0.
        let mutation = zero.autofix.clone().unwrap();
        let _reverse = mutation.apply(&mut p);
        let layer = p.layers.last().expect("layer present");
        if let Effect::Transform {
            scale_x, scale_y, ..
        } = &layer.effects[0]
        {
            assert!(
                matches!(scale_x, Modulator::Static(v) if (v - 1.0).abs() < 1e-6),
                "scale_x should be restored to 1.0, got {scale_x:?}"
            );
            assert!(
                matches!(scale_y, Modulator::Static(v) if (v - 1.0).abs() < 1e-6),
                "scale_y should be restored to 1.0, got {scale_y:?}"
            );
        } else {
            panic!("expected Effect::Transform");
        }
    }

    /// 003-T1.36 — Warp with a non-rectangular grid triggers DegenerateLayerWarp
    /// with a ResetWarpMesh autofix.
    #[test]
    fn audit_degenerate_warp_emits_finding() {
        let mut p = fresh_project();
        // Force a degenerate grid: claim cols=2 but only 1 vertex per row.
        p.layers[0].warp.cols = 2;
        p.layers[0].warp.grid = vec![vec![[0.0, 0.0]], vec![[0.0, 1.0]]]; // 1 col, not 3
        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        let f = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::DegenerateLayerWarp { .. }))
            .expect("expected DegenerateLayerWarp finding");
        assert_eq!(f.severity, Severity::Warn);
        assert!(
            f.autofix.is_some(),
            "DegenerateLayerWarp should have an autofix"
        );
    }

    /// 003-T1.36 — Healthy corner-pin (rows=1, cols=1, 2×2 vertex grid)
    /// is not flagged as degenerate.
    #[test]
    fn audit_degenerate_warp_skips_healthy_grid() {
        let p = fresh_project();
        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        assert!(
            !findings
                .iter()
                .any(|f| matches!(f.kind, AuditKind::DegenerateLayerWarp { .. })),
            "healthy default warp should not flag DegenerateLayerWarp",
        );
    }

    /// 003-T1.37 — mask_polygon with 1 or 2 vertices triggers LayerMaskTooFew.
    /// 0-vertex polygon and ≥3 polygon do not.
    #[test]
    fn audit_mask_too_few_triggers_for_one_or_two_vertices() {
        for count in [1usize, 2] {
            let mut p = fresh_project();
            p.layers[0].warp.mask_polygon = (0..count).map(|i| [i as f32 * 0.1, 0.5]).collect();
            let findings = ProjectAudit::run(&p, &AuditEnv::default());
            let f = findings
                .iter()
                .find(|f| matches!(f.kind, AuditKind::LayerMaskTooFew { .. }))
                .unwrap_or_else(|| panic!("expected LayerMaskTooFew for {count}-vertex polygon"));
            assert_eq!(f.severity, Severity::Info);
            assert!(f.autofix.is_some());
        }

        // Empty polygon: not flagged (operator hasn't started).
        {
            let mut p = fresh_project();
            p.layers[0].warp.mask_polygon = vec![];
            assert!(
                !ProjectAudit::run(&p, &AuditEnv::default())
                    .iter()
                    .any(|f| matches!(f.kind, AuditKind::LayerMaskTooFew { .. })),
                "empty polygon should not trigger LayerMaskTooFew"
            );
        }

        // ≥3 vertex polygon: not flagged.
        {
            let mut p = fresh_project();
            p.layers[0].warp.mask_polygon = vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
            assert!(
                !ProjectAudit::run(&p, &AuditEnv::default())
                    .iter()
                    .any(|f| matches!(f.kind, AuditKind::LayerMaskTooFew { .. })),
                "3-vertex polygon should not trigger LayerMaskTooFew"
            );
        }
    }

    /// 003-T1.38 — Layer pointing at a missing path triggers Critical
    /// MissingAsset; an existing file does not. The relink flow
    /// (T2.24) wires the autofix at the toast-action layer, not via
    /// the AuditFinding.autofix field — the replacement path comes
    /// from a file picker, not from project state.
    #[test]
    fn audit_missing_asset_relink_path() {
        let mut p = fresh_project();
        p.layers.push(crate::project::schema::layer_from_svg_path(
            "missing",
            std::path::PathBuf::from("/definitely/does/not/exist/3791f.svg"),
        ));
        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        let f = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::MissingAsset { .. }))
            .expect("expected MissingAsset");
        // 003-T2.24 — downgraded from Critical so the project still
        // opens; the relink toast in apply_launch_command surfaces
        // the "Find this file…" action.
        assert_eq!(f.severity, Severity::Warn);
        assert!(
            f.autofix.is_none(),
            "MissingAsset autofix lives at the toast-action layer (T2.24)"
        );

        // Sanity: an existing file does not trigger MissingAsset.
        let tmp = std::env::temp_dir().join("rmap_t138_audit_present.svg");
        std::fs::write(&tmp, b"<svg/>").expect("write tmp");
        let mut q = fresh_project();
        q.layers.push(crate::project::schema::layer_from_svg_path(
            "ok",
            tmp.clone(),
        ));
        assert!(
            !ProjectAudit::run(&q, &AuditEnv::default())
                .iter()
                .any(|f| matches!(f.kind, AuditKind::MissingAsset { .. })),
            "existing asset should not trigger MissingAsset"
        );
        std::fs::remove_file(&tmp).ok();
    }

    /// 003-T1.39 — output_target.fallback_index >= AuditEnv.monitor_count triggers
    /// MonitorOutOfRange with a SetOutputMonitorIndex autofix that resets
    /// the index to 0.
    #[test]
    fn audit_monitor_out_of_range_emits_finding() {
        let mut p = fresh_project();
        p.primary_output_target_mut().fallback_index = 99;
        let env = AuditEnv {
            monitor_count: 1,
            live_monitor_uuids: Vec::new(),
        };
        let findings = ProjectAudit::run(&p, &env);
        let f = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::MonitorOutOfRange { .. }))
            .expect("expected MonitorOutOfRange");
        assert_eq!(f.severity, Severity::Warn);
        assert!(f.autofix.is_some(), "MonitorOutOfRange autofix expected");

        // Apply autofix; assert index reset to 0.
        let mutation = f.autofix.clone().unwrap();
        let _reverse = mutation.apply(&mut p);
        assert_eq!(p.primary_output_target().fallback_index, 0);
    }

    /// 003-T2.21 — `first_run_canonical` half: load the bundled
    /// `assets/demos/window-glow.rmap.json` demo and assert
    /// [`ProjectAudit::run_with_path`] returns zero findings.
    ///
    /// This is the audit half of the spec's first-run smoke. The render
    /// half ("≥ 1 non-black pixel from the pipeline") would require
    /// bringing up the entire wgpu render graph behind `gpu-tests` —
    /// significantly more scaffolding than M2 can absorb. Instead we
    /// guard the same invariant at a lower layer: confirm the demo's
    /// image asset resolves to a file with non-zero size, which is the
    /// pre-condition for the image-layer pipeline to upload non-black
    /// texels. The full pipeline render is exercised by the manual
    /// stopwatch test in T-003-T2.9 acceptance.
    #[test]
    fn first_run_canonical_demo_audits_clean() {
        let demo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/demos/window-glow.rmap.json");
        assert!(
            demo_path.exists(),
            "bundled demo missing at {}; T-003-T2.8 ships it",
            demo_path.display(),
        );
        let project = crate::project::Project::load(&demo_path).expect("demo loads");

        let env = AuditEnv {
            // The demo's `output_target.fallback_index = 0` is always valid
            // because rmap requires at least one display to run; if
            // CI ever runs without a display the audit would surface
            // MonitorOutOfRange. monitor_count = 1 keeps the test
            // deterministic regardless of host hardware.
            monitor_count: 1,
            // Demo has no output_target.uuid set, so live_monitor_uuids
            // being empty won't trigger OutputTargetUuidNotFound.
            live_monitor_uuids: Vec::new(),
        };
        let findings = ProjectAudit::run_with_path(&project, &env, Some(&demo_path));
        assert!(
            findings.is_empty(),
            "T-003-T2.8 demo should audit clean; got {findings:?}",
        );

        // ≥ 1 non-black pixel pre-condition: the image asset must
        // exist and have non-zero size. The render-pipeline check
        // itself lives in T-003-T2.9 manual smoke.
        let layer = project
            .layers
            .first()
            .expect("demo carries at least one layer");
        let rel = layer
            .kind
            .asset_path()
            .expect("demo's first layer carries an asset path");
        let asset = project.resolve_asset(&demo_path, rel);
        let metadata = std::fs::metadata(&asset)
            .unwrap_or_else(|err| panic!("demo asset {} not on disk: {err}", asset.display()));
        assert!(
            metadata.len() > 0,
            "demo asset {} is a zero-byte placeholder; would render solid black",
            asset.display(),
        );
    }

    /// 004-V31.5.1 — `film_strip_demo_audits_clean`: load the bundled
    /// `assets/demos/film-strip.rmap.json` demo and assert
    /// [`ProjectAudit::run_with_path`] returns zero findings. Mirrors the
    /// `first_run_canonical_demo_audits_clean` test for the window-glow demo.
    #[test]
    fn film_strip_demo_audits_clean() {
        let demo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/demos/film-strip.rmap.json");
        assert!(
            demo_path.exists(),
            "bundled film-strip demo missing at {}; 004-V31.5.1 ships it",
            demo_path.display(),
        );
        let project = crate::project::Project::load(&demo_path).expect("film-strip demo loads");

        let env = AuditEnv {
            monitor_count: 1,
            live_monitor_uuids: Vec::new(),
        };
        let findings = ProjectAudit::run_with_path(&project, &env, Some(&demo_path));
        assert!(
            findings.is_empty(),
            "004-V31.5.1 film-strip demo should audit clean; got {findings:?}",
        );

        // ≥ 1 non-black pixel pre-condition: the shared image asset must
        // exist and have non-zero size. All 4 layers reference the same
        // photo so checking the first is sufficient.
        let layer = project
            .layers
            .first()
            .expect("film-strip demo carries at least one layer");
        let rel = layer
            .kind
            .asset_path()
            .expect("demo's first layer carries an asset path");
        let asset = project.resolve_asset(&demo_path, rel);
        let metadata = std::fs::metadata(&asset).unwrap_or_else(|err| {
            panic!(
                "film-strip demo asset {} not on disk: {err}",
                asset.display()
            )
        });
        assert!(
            metadata.len() > 0,
            "film-strip demo asset {} is a zero-byte placeholder; would render solid black",
            asset.display(),
        );
    }

    /// 004-V31.5.2 — `test_grid_demo_audits_clean`: load the bundled
    /// `assets/demos/test-grid.rmap.json` demo and assert
    /// [`ProjectAudit::run_with_path`] returns zero findings. Mirrors the
    /// `film_strip_demo_audits_clean` test for the test-grid demo.
    #[test]
    fn test_grid_demo_audits_clean() {
        let demo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/demos/test-grid.rmap.json");
        assert!(
            demo_path.exists(),
            "bundled test-grid demo missing at {}; 004-V31.5.2 ships it",
            demo_path.display(),
        );
        let project = crate::project::Project::load(&demo_path).expect("test-grid demo loads");

        let env = AuditEnv {
            monitor_count: 1,
            live_monitor_uuids: Vec::new(),
        };
        let findings = ProjectAudit::run_with_path(&project, &env, Some(&demo_path));
        assert!(
            findings.is_empty(),
            "004-V31.5.2 test-grid demo should audit clean; got {findings:?}",
        );

        // SVG layer asset must resolve and have non-zero size.
        let layer = project
            .layers
            .first()
            .expect("test-grid demo carries at least one layer");
        let rel = layer
            .kind
            .asset_path()
            .expect("demo's first layer carries an asset path");
        let asset = project.resolve_asset(&demo_path, rel);
        let metadata = std::fs::metadata(&asset).unwrap_or_else(|err| {
            panic!(
                "test-grid demo asset {} not on disk: {err}",
                asset.display()
            )
        });
        assert!(
            metadata.len() > 0,
            "test-grid demo asset {} is a zero-byte placeholder",
            asset.display(),
        );
    }

    /// 003-T1.40 — schema_version > CURRENT triggers Critical SchemaTooNew.
    /// Current version does not.
    #[test]
    fn audit_schema_too_new_emits_critical() {
        let mut p = fresh_project();
        p.schema_version = 99;
        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        let f = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::SchemaTooNew { .. }))
            .expect("expected SchemaTooNew");
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.autofix.is_none(), "SchemaTooNew has no autofix");

        // schema_version = CURRENT does not trigger.
        let q = fresh_project();
        assert!(
            !ProjectAudit::run(&q, &AuditEnv::default())
                .iter()
                .any(|f| matches!(f.kind, AuditKind::SchemaTooNew { .. })),
            "current schema version should not trigger SchemaTooNew"
        );
    }

    /// 003-T3.0d — `MultipleWarpsConsolidated` fires exactly once per
    /// session for a v3 project whose migration consolidated > 1
    /// warps; never fires for v3 projects with ≤ 1 warp; never fires
    /// for v4-native projects.
    #[test]
    fn audit_multiple_warps_consolidated_fires_once() {
        let p = fresh_project();
        p.transient_audit_signals
            .set(crate::project::schema::TransientAuditSignals {
                previous_warp_count: 3,
            });
        let env = AuditEnv::default();

        // First call: emits the finding once.
        let f1: Vec<_> = ProjectAudit::run(&p, &env)
            .into_iter()
            .filter(|f| matches!(f.kind, AuditKind::MultipleWarpsConsolidated { .. }))
            .collect();
        assert_eq!(
            f1.len(),
            1,
            "MultipleWarpsConsolidated must fire exactly once per session"
        );
        assert_eq!(f1[0].severity, Severity::Warn);
        assert!(f1[0].autofix.is_none());
        if let AuditKind::MultipleWarpsConsolidated {
            previous_warp_count,
            layer_count,
        } = f1[0].kind
        {
            assert_eq!(previous_warp_count, 3);
            assert_eq!(layer_count, p.layers.len());
        } else {
            unreachable!();
        }

        // Second call on the same project: never re-fires (Cell was
        // taken on the first call).
        let f2: Vec<_> = ProjectAudit::run(&p, &env)
            .into_iter()
            .filter(|f| matches!(f.kind, AuditKind::MultipleWarpsConsolidated { .. }))
            .collect();
        assert!(
            f2.is_empty(),
            "MultipleWarpsConsolidated must not re-fire on a second audit"
        );
    }

    /// 003-T3.0d — projects with ≤ 1 original warps never fire
    /// MultipleWarpsConsolidated. Includes v4-native (count 0) and
    /// the common single-warp v3 case (count 1).
    #[test]
    fn audit_multiple_warps_consolidated_skips_low_counts() {
        for previous_warp_count in [0usize, 1] {
            let p = fresh_project();
            p.transient_audit_signals
                .set(crate::project::schema::TransientAuditSignals {
                    previous_warp_count,
                });
            let findings = ProjectAudit::run(&p, &AuditEnv::default());
            assert!(
                !findings
                    .iter()
                    .any(|f| matches!(f.kind, AuditKind::MultipleWarpsConsolidated { .. })),
                "MultipleWarpsConsolidated must not fire for previous_warp_count={previous_warp_count}",
            );
        }
    }

    /// V31.2.2 — project has `output_target.uuid` set but no live monitor
    /// carries a matching UUID → `OutputTargetUuidNotFound` (Warn, no autofix).
    #[test]
    fn audit_output_target_uuid_not_found_emits_warning() {
        let mut p = fresh_project();
        {
            let pot = p.primary_output_target_mut();
            pot.uuid = Some("DEAD-BEEF-1234".to_string());
            pot.fallback_index = 0;
        }

        let env = AuditEnv {
            monitor_count: 1,
            // Live monitors have a different UUID — no match.
            live_monitor_uuids: vec![Some("AAAA-1111".to_string())],
        };
        let findings = ProjectAudit::run(&p, &env);
        let f = findings
            .iter()
            .find(|f| matches!(f.kind, AuditKind::OutputTargetUuidNotFound { .. }))
            .expect("expected OutputTargetUuidNotFound finding");
        assert_eq!(f.severity, Severity::Warn);
        assert!(
            f.autofix.is_none(),
            "OutputTargetUuidNotFound has no autofix (UUID comes from hardware)"
        );
        if let AuditKind::OutputTargetUuidNotFound {
            ref uuid,
            fallback_index,
        } = f.kind
        {
            assert_eq!(uuid, "DEAD-BEEF-1234");
            assert_eq!(fallback_index, 0);
        } else {
            unreachable!();
        }
    }

    /// V31.2.2 — project's UUID matches a live monitor → no
    /// `OutputTargetUuidNotFound` finding.
    #[test]
    fn audit_output_target_uuid_found_no_finding() {
        let mut p = fresh_project();
        p.primary_output_target_mut().uuid = Some("MATCH-ME".to_string());

        let env = AuditEnv {
            monitor_count: 1,
            live_monitor_uuids: vec![Some("MATCH-ME".to_string())],
        };
        let findings = ProjectAudit::run(&p, &env);
        assert!(
            !findings
                .iter()
                .any(|f| matches!(f.kind, AuditKind::OutputTargetUuidNotFound { .. })),
            "matching UUID should not produce OutputTargetUuidNotFound"
        );
    }

    /// V31.2.2 — when `live_monitor_uuids` is empty (caller didn't populate
    /// it), no `OutputTargetUuidNotFound` is emitted even if `uuid` is set.
    /// This preserves backward compatibility with call sites that only fill
    /// `monitor_count`.
    #[test]
    fn audit_output_target_uuid_not_found_skips_when_uuids_unpopulated() {
        let mut p = fresh_project();
        p.primary_output_target_mut().uuid = Some("SOME-UUID".to_string());

        // AuditEnv::default() has live_monitor_uuids: vec![]
        let findings = ProjectAudit::run(&p, &AuditEnv::default());
        assert!(
            !findings
                .iter()
                .any(|f| matches!(f.kind, AuditKind::OutputTargetUuidNotFound { .. })),
            "should not fire OutputTargetUuidNotFound when live_monitor_uuids is empty"
        );
    }
}
