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

    // P1.4.2 — always-on field normalisation. Runs before the
    // version-gated migrations and on the no-migration-needed path so
    // saves that already match `CURRENT_SCHEMA_VERSION` but were
    // written with an older field name (here: Video.loop_seamless) get
    // converted to the new field name (Video.loop_mode) before serde
    // parses the typed struct. This avoids bumping the schema version
    // for a strictly additive rename.
    normalize_video_loop_mode(&mut value);

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
        0..=6 => {
            if version <= 2 {
                migrate_v2_to_v3_layers(&mut value);
            }
            if version <= 3 {
                migrate_v3_to_v4_per_layer_warp(&mut value, &mut outcome);
            }
            if version <= 4 {
                migrate_v4_to_v5_warp_as_placement(&mut value);
            }
            if version <= 5 {
                migrate_v5_to_v6_output_target(&mut value);
            }
            // v6 → v7 (P0.1.2 + P0.7.1):
            //   • Adds `OutputTarget.rgb_matrix` (identity default via serde).
            //   • Adds `LayerKind::Video / FxLayer / Ndi` placeholder
            //     variants for W4 / W5 / W6.
            //   • **Renames `output_target: OutputTarget` →
            //     `output_targets: Vec<OutputTarget>`.** Wraps the
            //     prior singular value into a single-element vec so
            //     v6 projects load with a non-empty vec — the schema
            //     invariant `Project::primary_output_target()` relies
            //     on. Defensive: if `output_target` is missing, the
            //     serde default for `output_targets` populates a
            //     fresh single-element vec.
            if version <= 6 {
                migrate_v6_to_v7_output_targets(&mut value);
            }
            value["schema_version"] = serde_json::json!(CURRENT_SCHEMA_VERSION);
            Ok((value, outcome))
        }
        v => Err(ProjectError::UnsupportedVersion(v)),
    }
}

/// P1.4.2 — convert the legacy `LayerKind::Video.loop_seamless: bool`
/// field to `loop_mode: "Loop" | "Once"`. Walks every layer; on any
/// Video variant that carries `loop_seamless`, the field is removed
/// and `loop_mode` is set to the matching enum variant. Idempotent
/// (subsequent calls find no `loop_seamless` and do nothing).
fn normalize_video_loop_mode(value: &mut Value) {
    let Some(layers) = value.get_mut("layers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(kind) = layer.get_mut("kind") else {
            continue;
        };
        let Some(video) = kind.get_mut("Video").and_then(|v| v.as_object_mut()) else {
            continue;
        };
        if let Some(old) = video.remove("loop_seamless") {
            let variant = if old.as_bool().unwrap_or(true) {
                "Loop"
            } else {
                "Once"
            };
            // Don't overwrite a `loop_mode` that's already present
            // (some hand-edited project might carry both during a
            // half-applied edit; trust the new field).
            video
                .entry("loop_mode")
                .or_insert(Value::String(variant.into()));
        }
    }
}

/// Synthesize `LayerKind::Svg { svg_path }` for every v2 layer, removing the
/// old top-level `svg_path` field. v0/v1 layers also flow through this path:
/// at v0/v1 the same flat field existed, so the migration is identical.
/// Copy `Project.warps[0]` (or an identity warp) onto each layer's new
/// `warp` field, then drop the top-level `warps` array. Records the
/// original warp count in `outcome` so the audit pass (T3.0d) can fire
/// `MultipleWarpsConsolidated` exactly once when the migration was lossy
/// (M > 1 warps consolidated to N layers).
///
/// Also strips the per-warp `source_rect` field on copied warps —
/// schema v4 dropped it; serde silently ignores unknown fields, but
/// scrubbing keeps the migration output minimal.
fn migrate_v3_to_v4_per_layer_warp(value: &mut Value, outcome: &mut MigrationOutcome) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    let mut template_warp: Value = obj
        .get("warps")
        .and_then(|w| w.as_array())
        .map(|warps| {
            outcome.previous_warp_count = warps.len();
            warps.first().cloned().unwrap_or_else(default_identity_warp)
        })
        .unwrap_or_else(default_identity_warp);
    if let Some(t) = template_warp.as_object_mut() {
        t.remove("source_rect");
    }

    if let Some(layers) = obj.get_mut("layers").and_then(|v| v.as_array_mut()) {
        for layer in layers.iter_mut() {
            let Some(layer_obj) = layer.as_object_mut() else {
                continue;
            };
            // Preserve a hand-edited per-layer warp if one is already present
            // (someone may have written a v4-shaped file by hand and tagged
            // it `schema_version: 3`).
            if !layer_obj.contains_key("warp") {
                layer_obj.insert("warp".into(), template_warp.clone());
            } else if let Some(w) = layer_obj.get_mut("warp").and_then(|v| v.as_object_mut()) {
                w.remove("source_rect");
            }
        }
    }

    obj.remove("warps");
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
        "mask_polygon": [],
        "mask_feather": 0.02,
    })
}

/// 003-T3.29 — migrate v4 projects to the warp-as-placement model.
///
/// Pre-v5: warp grid corners were in projector [0, 1]² space and the
/// layer's `Effect::Transform` did the placement. New layers default
/// to a full-canvas warp (corners at the projector edges) and the
/// operator sized them via the Transform effect — leaving the warp
/// handles disconnected from the visible layer.
///
/// Post-v5: the warp grid IS the layer's placement on the projector.
/// New layers default to a half-size centered quad. Existing v4
/// projects migrate by reading each layer's first `Effect::Transform`
/// and synthesising warp corners that reproduce the same on-screen
/// quad, then resetting the Transform's `translate`/`scale_*` to
/// identity. `rotate_deg` is preserved on the Transform because
/// rotating the warp's four corners introduces a small visual delta
/// (the corners rotate but the layer image inside still rotates via
/// the Transform); leaving rotate alone keeps the migration visually
/// stable.
///
/// Three cases (per the T3.29 spec):
///   1. Identity grid + non-identity translate / scale (all-static
///      modulators) → synthesise corners; reset translate / scale.
///   2. Identity grid + identity translate / scale → replace grid
///      with the half-size centered default so the operator sees
///      the new model immediately.
///   3. Non-identity grid OR animated translate / scale → leave
///      unchanged. Either the operator authored a custom warp
///      already, or the layer is animation-driven; either way we
///      don't second-guess.
fn migrate_v4_to_v5_warp_as_placement(value: &mut Value) {
    let Some(layers) = value
        .as_object_mut()
        .and_then(|o| o.get_mut("layers"))
        .and_then(|v| v.as_array_mut())
    else {
        return;
    };

    for layer in layers.iter_mut() {
        let Some(layer_obj) = layer.as_object_mut() else {
            continue;
        };
        if !warp_grid_is_full_canvas(layer_obj.get("warp")) {
            continue;
        }

        let placement = read_static_transform_placement(layer_obj.get("effects"));
        let new_grid = match placement {
            // Case 1: scaled / translated → synthesise from the placement.
            Some(p) if !p.is_identity() => placement_to_grid(p),
            // Case 2: identity transform → centered half-size default.
            _ => default_placement_grid(),
        };

        if let Some(warp_obj) = layer_obj.get_mut("warp").and_then(|v| v.as_object_mut()) {
            warp_obj.insert("grid".into(), new_grid);
        }

        // Reset translate + scale on the first static Effect::Transform.
        // Only when we actually consumed a placement (case 1) — case 2
        // leaves Transform alone (it was already identity).
        if matches!(placement, Some(p) if !p.is_identity()) {
            reset_transform_placement(layer_obj.get_mut("effects"));
        }
    }
}

/// `true` when the warp's grid is the full-canvas identity quad
/// `[[0,0],[1,0]],[[0,1],[1,1]]`. We tolerate float jitter at 1e-4
/// because v3 → v4 wrote integer corners.
fn warp_grid_is_full_canvas(warp: Option<&Value>) -> bool {
    let Some(grid) = warp.and_then(|w| w.get("grid")).and_then(|g| g.as_array()) else {
        return false;
    };
    if grid.len() != 2 {
        return false;
    }
    let expected: [[(f64, f64); 2]; 2] = [[(0.0, 0.0), (1.0, 0.0)], [(0.0, 1.0), (1.0, 1.0)]];
    for (r, row) in grid.iter().enumerate() {
        let Some(row_arr) = row.as_array() else {
            return false;
        };
        if row_arr.len() != 2 {
            return false;
        }
        for (c, vert) in row_arr.iter().enumerate() {
            let Some(arr) = vert.as_array() else {
                return false;
            };
            if arr.len() != 2 {
                return false;
            }
            let x = arr[0].as_f64().unwrap_or(f64::NAN);
            let y = arr[1].as_f64().unwrap_or(f64::NAN);
            let (ex, ey) = expected[r][c];
            if (x - ex).abs() > 1e-4 || (y - ey).abs() > 1e-4 {
                return false;
            }
        }
    }
    true
}

/// Static placement read off the first `Effect::Transform` in a layer's
/// effects array. `None` when the effect is absent or any modulator is
/// non-static (animation case — leave alone per the T3.29 spec).
#[derive(Clone, Copy)]
struct StaticPlacement {
    translate: [f32; 2],
    scale_x: f32,
    scale_y: f32,
}

impl StaticPlacement {
    fn is_identity(&self) -> bool {
        self.translate[0].abs() < 1e-6
            && self.translate[1].abs() < 1e-6
            && (self.scale_x - 1.0).abs() < 1e-6
            && (self.scale_y - 1.0).abs() < 1e-6
    }
}

fn read_static_transform_placement(effects: Option<&Value>) -> Option<StaticPlacement> {
    let effects = effects?.as_array()?;
    for eff in effects {
        let Some(t) = eff.get("Transform") else {
            continue;
        };
        // translate is `[f32; 2]` (not a Modulator) — read directly.
        let translate = t
            .get("translate")
            .and_then(|v| v.as_array())
            .and_then(|a| Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32]))?;
        // scale_x / scale_y are Modulator enums; we only handle Static.
        let scale_x = static_modulator_value(t.get("scale_x"))?;
        let scale_y = static_modulator_value(t.get("scale_y"))?;
        return Some(StaticPlacement {
            translate,
            scale_x,
            scale_y,
        });
    }
    None
}

/// Read a `Static` modulator's f32 value. Returns `None` for any
/// non-Static variant so the migration falls through to "leave alone."
fn static_modulator_value(m: Option<&Value>) -> Option<f32> {
    let m = m?;
    // Modulator serializes as `{ "Static": <f32> }`. Other variants
    // serialize with their own keys (e.g. `{ "Sine": { … } }`).
    let s = m.get("Static")?;
    Some(s.as_f64()? as f32)
}

/// Convert `Effect::Transform`'s placement to a 2×2 warp grid.
/// `translate` is in the schema convention `[-1, 1]` (±1 = full screen
/// width/height); the layer's resulting bounding box in projector
/// `[0, 1]²` coords is `(0.5 + tx*0.5 ± 0.5*scale_x, 0.5 + ty*0.5 ±
/// 0.5*scale_y)`.
fn placement_to_grid(p: StaticPlacement) -> Value {
    let cx = 0.5 + p.translate[0] * 0.5;
    let cy = 0.5 + p.translate[1] * 0.5;
    let half_w = 0.5 * p.scale_x;
    let half_h = 0.5 * p.scale_y;
    let l = cx - half_w;
    let r = cx + half_w;
    let t = cy - half_h;
    let b = cy + half_h;
    serde_json::json!([[[l, t], [r, t]], [[l, b], [r, b]],])
}

/// JSON form of `WarpMesh::default_placement().grid` — half-size
/// centered quad used for case 2 of the migration.
fn default_placement_grid() -> Value {
    serde_json::json!([[[0.25, 0.25], [0.75, 0.25]], [[0.25, 0.75], [0.75, 0.75]],])
}

/// Reset the first `Effect::Transform`'s `translate` to `[0, 0]` and
/// `scale_x` / `scale_y` to `Static(1.0)`. `rotate_deg` is preserved.
/// No-op when no Transform effect is present or modulators are non-
/// static (the caller is expected to have gated on `read_static_…`).
fn reset_transform_placement(effects: Option<&mut Value>) {
    let Some(effects) = effects.and_then(|v| v.as_array_mut()) else {
        return;
    };
    for eff in effects.iter_mut() {
        let Some(t) = eff.get_mut("Transform") else {
            continue;
        };
        let Some(t_obj) = t.as_object_mut() else {
            continue;
        };
        t_obj.insert("translate".into(), serde_json::json!([0.0, 0.0]));
        t_obj.insert("scale_x".into(), serde_json::json!({ "Static": 1.0 }));
        t_obj.insert("scale_y".into(), serde_json::json!({ "Static": 1.0 }));
        return;
    }
}

/// P0.7.1 — migrate v6 projects to v7: replace the singular
/// `output_target: OutputTarget` with `output_targets:
/// Vec<OutputTarget>` (single-element wrap).
///
/// Defensive: if `output_target` is missing (already on v7 by some
/// other path, or malformed v6), serde's `default_output_targets`
/// populates a single default-target vec on deserialise.
fn migrate_v6_to_v7_output_targets(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let Some(legacy) = obj.remove("output_target") else {
        return;
    };
    obj.insert("output_targets".into(), Value::Array(vec![legacy]));
}

/// V31.2.1 — migrate v5 projects to v6: replace `output_monitor_index: usize`
/// with `output_target: { uuid: null, fallback_index: <prior index> }`.
///
/// Defensive: if `output_monitor_index` is missing (malformed v5 save),
/// defaults `fallback_index` to 0 and leaves the project loadable.
fn migrate_v5_to_v6_output_target(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let fallback_index = obj
        .remove("output_monitor_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    obj.insert(
        "output_target".into(),
        serde_json::json!({
            "uuid": null,
            "fallback_index": fallback_index,
        }),
    );
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
                    "rows": 2, "cols": 2,
                    "grid": [
                        [[0.0, 0.0], [0.5, 0.0], [1.0, 0.0]],
                        [[0.0, 0.5], [0.5, 0.5], [1.0, 0.5]],
                        [[0.0, 1.0], [0.5, 1.0], [1.0, 1.0]],
                    ],
                    "mask_polygon": [], "mask_feather": 0.05,
                }),
                serde_json::json!({
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "mask_polygon": [], "mask_feather": 0.01,
                }),
            ],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        // Top-level `warps` field is dropped from the migrated value.
        assert!(
            out.as_object()
                .map(|o| !o.contains_key("warps"))
                .unwrap_or(false)
        );
        let p: Project = serde_json::from_value(out).expect("deserialize as v4");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(outcome.previous_warp_count, 2);
        // Every layer received warps[0] (rows=2, feather=0.05 — not 1/0.01).
        for layer in &p.layers {
            assert_eq!(layer.warp.rows, 2);
            assert!((layer.warp.mask_feather - 0.05).abs() < 1e-6);
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

    /// T3.0a / T3.29 — v4-native projects flow through the v4 → v5 → v6
    /// steps. Empty-layers project bumps schema_version to 6 with no
    /// other changes; outcome reports `previous_warp_count == 0` so
    /// the audit finding never fires for fresh projects.
    #[test]
    fn migrate_v4_native_passes_through() {
        let v = serde_json::json!({
            "schema_version": 4,
            "layers": [],
            "warps": [],
        });
        let (out, outcome) = migrate(v).expect("migrate");
        assert_eq!(
            out["schema_version"],
            serde_json::json!(CURRENT_SCHEMA_VERSION)
        );
        assert_eq!(outcome.previous_warp_count, 0);
    }

    /// T3.29 — a v4 layer with full-canvas warp + scaled / translated
    /// `Effect::Transform` migrates to a quad warp matching the
    /// pre-migration on-screen placement, with the Transform's
    /// translate / scale reset to identity.
    #[test]
    fn migrate_v4_to_v5_synthesises_grid_from_static_transform() {
        // Layer scaled to half-size, no translate. Expected: warp
        // grid corners at (0.25 … 0.75) and Transform reset.
        let v = serde_json::json!({
            "schema_version": 4,
            "layers": [{
                "id": "scaled",
                "kind": { "Image": { "path": "/tmp/x.png", "fit": "Cover", "focal": [0.5, 0.5] } },
                "enabled": true,
                "transform": {
                    "translate": [0.0, 0.0], "rotate_deg": 0.0,
                    "scale": [1.0, 1.0], "anchor": [0.5, 0.5]
                },
                "effects": [
                    { "Transform": {
                        "translate": [0.0, 0.0],
                        "rotate_deg": { "Static": 0.0 },
                        "scale_x": { "Static": 0.5 },
                        "scale_y": { "Static": 0.5 }
                    }}
                ],
                "blend_mode": "Normal",
                "opacity": 1.0,
                "warp": {
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "mask_polygon": [],
                    "mask_feather": 0.02
                }
            }]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize migrated project");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);

        // Synthesised quad: corners at (0.25 … 0.75) for scale 0.5.
        let g = &p.layers[0].warp.grid;
        let approx = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(approx(g[0][0][0], 0.25) && approx(g[0][0][1], 0.25));
        assert!(approx(g[0][1][0], 0.75) && approx(g[0][1][1], 0.25));
        assert!(approx(g[1][0][0], 0.25) && approx(g[1][0][1], 0.75));
        assert!(approx(g[1][1][0], 0.75) && approx(g[1][1][1], 0.75));

        // Transform's translate / scale reset to identity; the rest
        // (rotate, anchor) preserved.
        let eff = &p.layers[0].effects[0];
        let crate::effects::Effect::Transform {
            translate,
            scale_x,
            scale_y,
            ..
        } = eff
        else {
            panic!("expected Effect::Transform after migration, got {eff:?}");
        };
        assert_eq!(*translate, [0.0, 0.0]);
        match scale_x {
            crate::modulators::Modulator::Static(v) => assert!(approx(*v, 1.0)),
            other => panic!("scale_x should be Static(1.0), got {other:?}"),
        }
        match scale_y {
            crate::modulators::Modulator::Static(v) => assert!(approx(*v, 1.0)),
            other => panic!("scale_y should be Static(1.0), got {other:?}"),
        }
    }

    /// T3.29 — a v4 layer with full-canvas warp + identity Transform
    /// gets the half-size centered default warp so the new model is
    /// immediately visible to the operator.
    #[test]
    fn migrate_v4_to_v5_identity_transform_gets_default_placement() {
        let v = serde_json::json!({
            "schema_version": 4,
            "layers": [{
                "id": "fresh",
                "kind": { "Image": { "path": "/tmp/x.png", "fit": "Cover", "focal": [0.5, 0.5] } },
                "enabled": true,
                "transform": {
                    "translate": [0.0, 0.0], "rotate_deg": 0.0,
                    "scale": [1.0, 1.0], "anchor": [0.5, 0.5]
                },
                "effects": [
                    { "Transform": {
                        "translate": [0.0, 0.0],
                        "rotate_deg": { "Static": 0.0 },
                        "scale_x": { "Static": 1.0 },
                        "scale_y": { "Static": 1.0 }
                    }}
                ],
                "blend_mode": "Normal",
                "opacity": 1.0,
                "warp": {
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "mask_polygon": [],
                    "mask_feather": 0.02
                }
            }]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v5");
        let g = &p.layers[0].warp.grid;
        let approx = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(approx(g[0][0][0], 0.25) && approx(g[0][0][1], 0.25));
        assert!(approx(g[1][1][0], 0.75) && approx(g[1][1][1], 0.75));
    }

    // --- V31.2.1 migration tests ---

    fn minimal_v5_json(output_monitor_index: u64) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 5,
            "layers": [],
            "output_monitor_index": output_monitor_index,
        })
    }

    /// V31.2.1 — v5 project with `output_monitor_index: 2` migrates to
    /// `output_target: { uuid: null, fallback_index: 2 }` at v6.
    #[test]
    fn v5_output_monitor_index_migrates_to_output_target_with_null_uuid() {
        let v = minimal_v5_json(2);
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v6");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p.primary_output_target().uuid, None);
        assert_eq!(p.primary_output_target().fallback_index, 2);
    }

    /// V31.2.1 — v5 project with `output_monitor_index: 0` migrates correctly.
    #[test]
    fn v5_with_index_zero_migrates_correctly() {
        let v = minimal_v5_json(0);
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v6");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p.primary_output_target().uuid, None);
        assert_eq!(p.primary_output_target().fallback_index, 0);
    }

    /// P0.7.1 — v6 → v7 migration wraps the singular `output_target`
    /// into a `output_targets` single-element vec.
    #[test]
    fn v6_to_v7_wraps_output_target_into_vec() {
        let v = serde_json::json!({
            "schema_version": 6,
            "layers": [],
            "output_target": {
                "uuid": "TEST-UUID",
                "fallback_index": 1,
            },
        });
        let (out, _) = migrate(v).expect("migrate");
        // Raw JSON form: `output_target` is gone, `output_targets`
        // is a single-element array carrying the prior values.
        assert!(out.get("output_target").is_none(), "singular field removed");
        let arr = out
            .get("output_targets")
            .and_then(|v| v.as_array())
            .expect("output_targets is an array");
        assert_eq!(arr.len(), 1, "single-element wrap");
        assert_eq!(arr[0]["uuid"], "TEST-UUID");
        assert_eq!(arr[0]["fallback_index"], 1);
    }

    /// P0.7.1 — defensive: v6 project with `output_target` missing
    /// (malformed save) loads via the serde default (single default
    /// target), preserving the schema invariant
    /// `output_targets` is non-empty.
    #[test]
    fn v6_with_missing_output_target_falls_back_to_default() {
        let v = serde_json::json!({
            "schema_version": 6,
            "layers": [],
            // deliberately omit output_target
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        assert_eq!(p.output_targets.len(), 1, "default populates one target");
    }

    /// V31.2.1 — a v6 project with `output_target` already present is
    /// migrated to v7 by the migration chain (P0.1.2). The output_target
    /// data is preserved verbatim; the new `rgb_matrix` field is
    /// populated with the identity matrix via serde's default.
    #[test]
    fn v6_with_output_target_migrates_to_v7() {
        let v = serde_json::json!({
            "schema_version": 6,
            "layers": [],
            "output_target": {
                "uuid": null,
                "fallback_index": 3,
            },
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v7");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p.primary_output_target().uuid, None);
        assert_eq!(p.primary_output_target().fallback_index, 3);
        assert_eq!(
            p.primary_output_target().rgb_matrix,
            crate::project::schema::rgb_matrix_identity(),
            "v6 → v7 migration must default rgb_matrix to identity",
        );
    }

    /// P0.1.2 — a v7 project with all three new `LayerKind` variants
    /// round-trips through the migration chain unchanged.
    #[test]
    fn v7_with_new_layer_kinds_round_trips() {
        let v = serde_json::json!({
            "schema_version": 7,
            "layers": [
                {
                    "id": "vid",
                    "kind": { "Video": { "path": "/tmp/clip.mp4" } },
                    "enabled": true,
                    "transform": {
                        "translate": [0.0, 0.0],
                        "rotate_deg": 0.0,
                        "scale": [1.0, 1.0],
                        "anchor": [0.5, 0.5],
                    },
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1,
                        "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    },
                },
                {
                    "id": "fx",
                    "kind": { "FxLayer": { "preset_id": "ripple_wash" } },
                    "enabled": true,
                    "transform": {
                        "translate": [0.0, 0.0],
                        "rotate_deg": 0.0,
                        "scale": [1.0, 1.0],
                        "anchor": [0.5, 0.5],
                    },
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1,
                        "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    },
                },
                {
                    "id": "ndi",
                    "kind": { "Ndi": { "source_name": "OBS-NDI" } },
                    "enabled": true,
                    "transform": {
                        "translate": [0.0, 0.0],
                        "rotate_deg": 0.0,
                        "scale": [1.0, 1.0],
                        "anchor": [0.5, 0.5],
                    },
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1,
                        "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    },
                },
            ],
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v7");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p.layers.len(), 3);
        assert!(matches!(
            p.layers[0].kind,
            crate::project::schema::LayerKind::Video { .. }
        ));
        assert!(matches!(
            p.layers[1].kind,
            crate::project::schema::LayerKind::FxLayer { .. }
        ));
        assert!(matches!(
            p.layers[2].kind,
            crate::project::schema::LayerKind::Ndi { .. }
        ));
        // asset_path semantics: Video has one, FxLayer + Ndi do not.
        assert!(p.layers[0].kind.asset_path().is_some());
        assert!(p.layers[1].kind.asset_path().is_none());
        assert!(p.layers[2].kind.asset_path().is_none());
    }

    /// V31.2.1 — defensive: a malformed v5 project missing `output_monitor_index`
    /// defaults to `fallback_index: 0`.
    #[test]
    fn v5_missing_output_monitor_index_defaults_to_zero() {
        let v = serde_json::json!({
            "schema_version": 5,
            "layers": [],
            // deliberately omit output_monitor_index
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v6");
        assert_eq!(p.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(p.primary_output_target().uuid, None);
        assert_eq!(p.primary_output_target().fallback_index, 0);
    }

    /// T3.29 — a v4 layer with a custom (non-identity) warp grid is
    /// left alone — operator already authored a placement.
    #[test]
    fn migrate_v4_to_v5_custom_grid_left_alone() {
        let custom_grid = serde_json::json!([[[0.1, 0.1], [0.9, 0.1]], [[0.1, 0.9], [0.9, 0.9]]]);
        let v = serde_json::json!({
            "schema_version": 4,
            "layers": [{
                "id": "authored",
                "kind": { "Image": { "path": "/tmp/x.png", "fit": "Cover", "focal": [0.5, 0.5] } },
                "enabled": true,
                "transform": {
                    "translate": [0.0, 0.0], "rotate_deg": 0.0,
                    "scale": [1.0, 1.0], "anchor": [0.5, 0.5]
                },
                "effects": [],
                "blend_mode": "Normal",
                "opacity": 1.0,
                "warp": {
                    "rows": 1, "cols": 1,
                    "grid": custom_grid.clone(),
                    "mask_polygon": [],
                    "mask_feather": 0.02
                }
            }]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize as v5");
        let g = &p.layers[0].warp.grid;
        assert!((g[0][0][0] - 0.1).abs() < 1e-4);
        assert!((g[1][1][0] - 0.9).abs() < 1e-4);
    }

    /// P1.4.2 — a v7 save written before the loop_seamless → loop_mode
    /// rename loads with the correct enum variant (always-on
    /// normalisation pass; no schema-version bump).
    #[test]
    fn loop_seamless_true_normalises_to_loop_mode_loop() {
        let v = serde_json::json!({
            "schema_version": CURRENT_SCHEMA_VERSION,
            "layers": [{
                "id": "v0",
                "kind": { "Video": { "path": "/tmp/x.mp4", "speed": 1.0, "loop_seamless": true } },
                "enabled": true,
                "transform": {
                    "translate": [0.0, 0.0], "rotate_deg": 0.0,
                    "scale": [1.0, 1.0], "anchor": [0.5, 0.5]
                },
                "effects": [],
                "blend_mode": "Normal",
                "opacity": 1.0,
                "warp": {
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "mask_polygon": [], "mask_feather": 0.02
                }
            }]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        match &p.layers[0].kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => {
                assert_eq!(
                    *loop_mode,
                    crate::project::schema::LoopMode::Loop,
                    "loop_seamless: true should normalise to LoopMode::Loop"
                );
            }
            _ => panic!("expected Video layer"),
        }
    }

    /// P1.4.2 — `loop_seamless: false` normalises to `LoopMode::Once`.
    #[test]
    fn loop_seamless_false_normalises_to_loop_mode_once() {
        let v = serde_json::json!({
            "schema_version": CURRENT_SCHEMA_VERSION,
            "layers": [{
                "id": "v0",
                "kind": { "Video": { "path": "/tmp/x.mp4", "speed": 1.0, "loop_seamless": false } },
                "enabled": true,
                "transform": {
                    "translate": [0.0, 0.0], "rotate_deg": 0.0,
                    "scale": [1.0, 1.0], "anchor": [0.5, 0.5]
                },
                "effects": [],
                "blend_mode": "Normal",
                "opacity": 1.0,
                "warp": {
                    "rows": 1, "cols": 1,
                    "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                    "mask_polygon": [], "mask_feather": 0.02
                }
            }]
        });
        let (out, _) = migrate(v).expect("migrate");
        let p: Project = serde_json::from_value(out).expect("deserialize");
        match &p.layers[0].kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => {
                assert_eq!(
                    *loop_mode,
                    crate::project::schema::LoopMode::Once,
                    "loop_seamless: false should normalise to LoopMode::Once"
                );
            }
            _ => panic!("expected Video layer"),
        }
    }
}
