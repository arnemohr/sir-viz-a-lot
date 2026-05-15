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
        0..=11 => {
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
            // v7 → v8 (P3.2.2):
            //   • Adds `WarpMesh.zone_role: null` to every layer's warp
            //     object that lacks it. Technically a no-op for serde
            //     (the field defaults to None), but the explicit migration
            //     step lets audit tooling report "migrated from v7" and
            //     future phases reason about when zone_role first appeared.
            if version <= 7 {
                migrate_v7_to_v8_zone_role(&mut value);
            }
            // v8 → v9 (P6.2.3):
            //   • Renames `scenes` → `cues` in the JSON object.
            //   • Injects identity defaults for all new Cue timing fields:
            //     `in_time_s: 0.0`, `hold_time_s: null`, `out_time_s: 0.0`,
            //     `fire_mode: "GoOnTrigger"`, `bpm_quantize: "Off"`,
            //     `timecode_trigger: null`, and all binding fields: null.
            //   • Projects saved with v9's `cues` key load correctly via
            //     the `#[serde(alias = "scenes")]` attribute without this
            //     migration; the step is present so saved files are written
            //     with `cues` going forward and future tooling can detect
            //     the version where cuelist timing was first supported.
            if version <= 8 {
                migrate_v8_to_v9_scenes_to_cues(&mut value);
            }
            // v9 → v10 (P7.3.1):
            //   • Adds `LayerConfig.bezier_mesh` by converting each layer's
            //     existing `warp` grid into a `BezierMesh` with all handles
            //     `None` (bilinear-equivalent). Existing `warp` field is
            //     preserved for backward-compat rendering; `bezier_mesh` is
            //     `Some` for migrated projects and drives the Bezier render
            //     path once P7.3.2 wires it.
            if version <= 9 {
                migrate_v9_to_v10_bezier_mesh(&mut value);
            }
            // v10 → v11 (P7.4.1):
            //   • Adds `LayerConfig.mask_graph` by converting each layer's
            //     `bezier_mesh.mask_polygon` + `bezier_mesh.mask_feather`
            //     (or `warp.mask_polygon` + `warp.mask_feather` for layers
            //     without a `bezier_mesh`) into a single-node `MaskGraph`.
            //   • `bezier_mesh.mask_polygon` and `bezier_mesh.mask_feather`
            //     are left in place for backward compat but are superseded
            //     by `mask_graph` for rendering.
            if version <= 10 {
                migrate_v10_to_v11_mask_graph(&mut value);
            }
            // v11 → v12 (004-T1.3/T1.4):
            //   • Drops `LayerConfig.treatment` — the optional Treatment
            //     object is folded into the per-layer `effects` chain as the
            //     first element (if `preset_id` is non-empty).
            //   • Each existing `effects[i]` plain-effect is wrapped in
            //     `{"enabled": true, "effect": <old>}` (EffectNode shape).
            if version <= 11 {
                migrate_v11_to_v12_fold_treatment_into_effects(&mut value);
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

// ---------------------------------------------------------------------------
// P6.2.3 — v8 → v9: rename `scenes` → `cues`, inject timing defaults.
// ---------------------------------------------------------------------------

/// P6.2.3 — Rename the `scenes` key to `cues` in the JSON object and
/// inject identity defaults for all Phase 6 timing fields that are absent.
///
/// The `#[serde(alias = "scenes")]` on `Project.cues` means old files still
/// load without this migration — the migration step ensures files saved after
/// v9 use `cues` and lets future tooling detect the version boundary.
fn migrate_v8_to_v9_scenes_to_cues(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    // Rename `scenes` → `cues` if present.
    if let Some(scenes_val) = obj.remove("scenes") {
        obj.insert("cues".to_string(), scenes_val);
    }
    // Inject identity timing defaults into each cue entry.
    if let Some(Value::Array(cues)) = obj.get_mut("cues") {
        for cue in cues.iter_mut() {
            let Some(cue_obj) = cue.as_object_mut() else {
                continue;
            };
            cue_obj
                .entry("in_time_s")
                .or_insert(Value::Number(serde_json::Number::from_f64(0.0).unwrap()));
            cue_obj.entry("hold_time_s").or_insert(Value::Null);
            cue_obj
                .entry("out_time_s")
                .or_insert(Value::Number(serde_json::Number::from_f64(0.0).unwrap()));
            cue_obj
                .entry("fire_mode")
                .or_insert(Value::String("GoOnTrigger".into()));
            cue_obj
                .entry("bpm_quantize")
                .or_insert(Value::String("Off".into()));
            cue_obj.entry("timecode_trigger").or_insert(Value::Null);
            cue_obj.entry("in_time_binding").or_insert(Value::Null);
            cue_obj.entry("hold_binding").or_insert(Value::Null);
            cue_obj.entry("out_time_binding").or_insert(Value::Null);
            cue_obj.entry("in_time_osc").or_insert(Value::Null);
            cue_obj.entry("hold_osc").or_insert(Value::Null);
            cue_obj.entry("out_time_osc").or_insert(Value::Null);
        }
    }
}

// ---------------------------------------------------------------------------
// P7.3.1 — v9 → v10: add `bezier_mesh` from existing `warp` grid.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// P7.4.1 — v10 → v11: add `mask_graph` from existing mask_polygon fields.
// ---------------------------------------------------------------------------

/// P7.4.1 — For every layer, convert `bezier_mesh.mask_polygon` +
/// `bezier_mesh.mask_feather` (or `warp.mask_polygon` + `warp.mask_feather`
/// for layers without a `bezier_mesh`) into a single-node `MaskGraph`.
///
/// Idempotent: layers that already have a non-null `mask_graph` are skipped.
///
/// ## MaskGraph JSON layout
/// ```json
/// {
///   "nodes": [
///     { "kind": "Polygon", "points": <mask_polygon>, "feather": <mask_feather> }
///   ]
/// }
/// ```
fn migrate_v10_to_v11_mask_graph(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let Some(Value::Array(layers)) = obj.get_mut("layers") else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(layer_obj) = layer.as_object_mut() else {
            continue;
        };
        // Skip layers that already have a mask_graph (idempotent).
        if layer_obj.contains_key("mask_graph") && layer_obj["mask_graph"] != Value::Null {
            continue;
        }
        // Prefer mask data from `bezier_mesh` (populated by v9→v10); fall
        // back to `warp` for layers that somehow lack a `bezier_mesh`.
        let (mask_polygon, mask_feather) =
            if let Some(bm) = layer_obj.get("bezier_mesh").and_then(|v| v.as_object()) {
                (
                    bm.get("mask_polygon")
                        .cloned()
                        .unwrap_or(Value::Array(vec![])),
                    bm.get("mask_feather")
                        .cloned()
                        .unwrap_or(serde_json::json!(0.02)),
                )
            } else if let Some(warp) = layer_obj.get("warp").and_then(|v| v.as_object()) {
                (
                    warp.get("mask_polygon")
                        .cloned()
                        .unwrap_or(Value::Array(vec![])),
                    warp.get("mask_feather")
                        .cloned()
                        .unwrap_or(serde_json::json!(0.02)),
                )
            } else {
                (Value::Array(vec![]), serde_json::json!(0.02))
            };

        let mask_graph = serde_json::json!({
            "nodes": [
                {
                    "kind": "Polygon",
                    "points": mask_polygon,
                    "feather": mask_feather
                }
            ]
        });
        layer_obj.insert("mask_graph".to_string(), mask_graph);
    }
}

/// P7.3.1 — For every layer that has a `warp` object but no `bezier_mesh`,
/// synthesise a `BezierMesh` JSON value from the warp grid with all handles
/// `None`.  The existing `warp` field is preserved so the bilinear render path
/// keeps working; `bezier_mesh` becomes `Some` and the Bezier render path
/// (P7.3.2) dispatches on its presence.
///
/// ## BezierMesh JSON layout
/// ```json
/// {
///   "rows": <same as warp.rows>,
///   "cols": <same as warp.cols>,
///   "anchors": <same as warp.grid>,
///   "handles_h": <(rows+1) × (cols+1) nulls>,
///   "handles_v": <(rows+1) × (cols+1) nulls>,
///   "mask_polygon": <same as warp.mask_polygon>,
///   "mask_feather": <same as warp.mask_feather>,
///   "zone_role": <same as warp.zone_role>
/// }
/// ```
fn migrate_v9_to_v10_bezier_mesh(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let Some(Value::Array(layers)) = obj.get_mut("layers") else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(layer_obj) = layer.as_object_mut() else {
            continue;
        };
        // Skip layers that already have a bezier_mesh (idempotent).
        if layer_obj.contains_key("bezier_mesh") && layer_obj["bezier_mesh"] != Value::Null {
            continue;
        }
        // Only migrate layers that have a `warp` object.
        let Some(warp) = layer_obj.get("warp").cloned() else {
            continue;
        };
        let Some(warp_obj) = warp.as_object() else {
            continue;
        };

        let rows = warp_obj.get("rows").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let cols = warp_obj.get("cols").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let grid = warp_obj
            .get("grid")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        let mask_polygon = warp_obj
            .get("mask_polygon")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        let mask_feather = warp_obj
            .get("mask_feather")
            .cloned()
            .unwrap_or(serde_json::json!(0.02));
        let zone_role = warp_obj.get("zone_role").cloned().unwrap_or(Value::Null);

        // Build all-None handles: (rows+1) × (cols+1) nulls.
        let null_row: Vec<Value> = vec![Value::Null; cols + 1];
        let handles: Vec<Value> = vec![Value::Array(null_row); rows + 1];

        let bezier_mesh = serde_json::json!({
            "rows": rows,
            "cols": cols,
            "anchors": grid,
            "handles_h": handles.clone(),
            "handles_v": handles,
            "mask_polygon": mask_polygon,
            "mask_feather": mask_feather,
            "zone_role": zone_role,
        });
        layer_obj.insert("bezier_mesh".to_string(), bezier_mesh);
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

/// P3.2.2 — migrate v7 projects to v8: ensure every layer's `warp` object
/// has a `zone_role: null` key.
///
/// This migration is technically a no-op for serde — the `#[serde(default)]`
/// attribute on `WarpMesh.zone_role` already maps an absent key to `None`.
/// The explicit migration step ensures the audit log can report "migrated
/// from v7" and future tooling can reason about which version introduced
/// `zone_role`.
fn migrate_v7_to_v8_zone_role(value: &mut Value) {
    let Some(layers) = value.get_mut("layers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(layer_obj) = layer.as_object_mut() else {
            continue;
        };
        let Some(warp) = layer_obj.get_mut("warp").and_then(|w| w.as_object_mut()) else {
            continue;
        };
        warp.entry("zone_role").or_insert(Value::Null);
    }
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

// ---------------------------------------------------------------------------
// 004-T1.4 — v11 → v12: fold `treatment` into `effects` as the first node
// ---------------------------------------------------------------------------

/// 004-T1.4 — Migrate a project from schema v11 to v12.
///
/// For each layer:
///   1. Read `treatment` (default `null`) and `effects` (default `[]`).
///   2. If `treatment` is an object with a non-empty `preset_id`, prepend
///      `{"enabled": true, "effect": {"Treatment": {"id": <preset_id>,
///      "params": <params or {}>, "overlay_path": <overlay_path or null>,
///      "collage_paths": <collage_paths or []>}}}` to the new effects vec.
///   3. Wrap each existing effect as `{"enabled": true, "effect": <existing>}`.
///   4. Replace `layer["effects"]` with the new vec; remove `layer["treatment"]`.
///
/// The migrator is disk-blind: it does not read any files referenced by
/// `overlay_path` or `collage_paths`. Missing or invalid paths are carried
/// verbatim into the EffectNode and will surface as audit findings at load time.
///
/// Idempotent on v12 input (effects already have the `enabled`/`effect` shape;
/// `treatment` key is absent; the step is a no-op).
fn migrate_v11_to_v12_fold_treatment_into_effects(value: &mut Value) {
    let Some(layers) = value.get_mut("layers").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for layer in layers.iter_mut() {
        let Some(obj) = layer.as_object_mut() else {
            continue;
        };

        // Extract treatment (may be null, absent, or an object).
        let treatment = obj.remove("treatment").unwrap_or(Value::Null);

        // Extract existing effects array (may be absent → treat as empty).
        let old_effects = obj
            .remove("effects")
            .and_then(|v| if v.is_array() { Some(v) } else { None })
            .unwrap_or_else(|| Value::Array(vec![]));
        let old_effects_arr = old_effects.as_array().expect("checked above");

        let mut new_effects: Vec<Value> = Vec::new();

        // Step 2 — prepend Treatment node if treatment.preset_id is non-empty.
        if let Some(t_obj) = treatment.as_object() {
            let preset_id = t_obj
                .get("preset_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !preset_id.is_empty() {
                let params = t_obj
                    .get("params")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let overlay_path = t_obj.get("overlay_path").cloned().unwrap_or(Value::Null);
                let collage_paths = t_obj
                    .get("collage_paths")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([]));
                new_effects.push(serde_json::json!({
                    "enabled": true,
                    "effect": {
                        "Treatment": {
                            "id": preset_id,
                            "params": params,
                            "overlay_path": overlay_path,
                            "collage_paths": collage_paths
                        }
                    }
                }));
            }
        }

        // Step 3 — wrap each existing effect as EffectNode.
        // Skip effects that already look like EffectNode (have "enabled" + "effect" keys)
        // so the migration is idempotent on v12 projects that happen to round-trip through
        // this path.
        for eff in old_effects_arr {
            if eff.get("enabled").is_some() && eff.get("effect").is_some() {
                // Already in EffectNode shape — copy verbatim (idempotent path).
                new_effects.push(eff.clone());
            } else {
                new_effects.push(serde_json::json!({
                    "enabled": true,
                    "effect": eff
                }));
            }
        }

        obj.insert("effects".into(), Value::Array(new_effects));
        // `treatment` was already removed by obj.remove("treatment") above.
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
        let eff = &p.layers[0].effects[0].effect;
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

    // --- P3.2.2 schema migration v7 → v8 tests ---

    /// 004-T1.3 — `CURRENT_SCHEMA_VERSION == 12` (bumped from 11 by 004-T1.3).
    #[test]
    fn current_schema_version_is_12() {
        assert_eq!(
            CURRENT_SCHEMA_VERSION, 12,
            "CURRENT_SCHEMA_VERSION must be 12 after 004-T1.3"
        );
    }

    /// P3.2.2 — a v7 project JSON (without `zone_role` keys) migrates cleanly
    /// to v8 with `zone_role: null` on every warp.
    #[test]
    fn migrate_v7_to_v8_adds_zone_role_null() {
        let v7_json = serde_json::json!({
            "schema_version": 7u32,
            "layers": [
                {
                    "id": "img0",
                    "kind": {"Image": {"path": "/tmp/photo.jpg", "fit": "Cover", "focal": [0.5, 0.5]}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1, "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [], "mask_feather": 0.02
                    }
                },
                {
                    "id": "fx0",
                    "kind": {"FxLayer": {"preset_id": "mask_edge_ripple_wash", "params": {}}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1, "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]],
                        "mask_feather": 0.02
                    }
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        let (migrated, _) = migrate(v7_json).expect("migrate v7 → v9");

        // schema_version must be CURRENT_SCHEMA_VERSION (9).
        assert_eq!(migrated["schema_version"], CURRENT_SCHEMA_VERSION);

        // Every warp must have zone_role: null.
        for layer in migrated["layers"].as_array().unwrap() {
            let zone_role = &layer["warp"]["zone_role"];
            assert!(
                zone_role.is_null(),
                "zone_role must be null after v7→v8 migration, got {zone_role}"
            );
        }
    }

    /// P3.2.2 — a v8 project JSON with an explicit `zone_role: "window"` on
    /// one layer round-trips through `migrate()` unchanged.
    #[test]
    fn migrate_v8_with_zone_role_unchanged() {
        let v8_json = serde_json::json!({
            "schema_version": 8u32,
            "layers": [
                {
                    "id": "img0",
                    "kind": {"Image": {"path": "/tmp/photo.jpg", "fit": "Cover", "focal": [0.5, 0.5]}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1, "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [], "mask_feather": 0.02,
                        "zone_role": "window"
                    }
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        // v8 projects are migrated to v10 (cascading through v9 + v10 steps).
        let (migrated, _) = migrate(v8_json).expect("migrate v8 → v10 (cascade)");
        assert_eq!(migrated["schema_version"], CURRENT_SCHEMA_VERSION);
        assert_eq!(
            migrated["layers"][0]["warp"]["zone_role"], "window",
            "zone_role must not be modified by the migration chain"
        );
    }

    // --- P3.6.3 old-project regression test ---

    /// P3.6.3 — A v7 project without any `zone_role` keys migrates to v8
    /// identically: `zone_role = None` on all layers, schema_version == 8,
    /// and no zone-related audit findings. CPU-only test (no GPU).
    #[cfg(feature = "v3")]
    #[test]
    fn old_v7_project_loads_identically_after_migration() {
        use crate::project::audit::{AuditEnv, AuditKind, ProjectAudit};
        use crate::project::schema::{CURRENT_SCHEMA_VERSION, Project};

        // Construct a minimal v7 project JSON: one Image layer, one FxLayer,
        // no `zone_role` keys anywhere — simulates a pre-P3 saved project.
        let v7_json = serde_json::json!({
            "schema_version": 7u32,
            "layers": [
                {
                    "id": "img0",
                    "kind": {"Image": {"path": "/tmp/photo.jpg", "fit": "Cover", "focal": [0.5, 0.5]}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1, "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [], "mask_feather": 0.02
                    }
                },
                {
                    "id": "fx0",
                    "kind": {"FxLayer": {"preset_id": "mask_edge_ripple_wash", "params": {}}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1, "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]],
                        "mask_feather": 0.02
                    }
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        // Migrate the v7 project to v9 (passes through v8 and v9 migration steps).
        let (migrated, _) = migrate(v7_json).expect("migrate v7 → v9");

        // Assertion 1: schema_version == CURRENT_SCHEMA_VERSION (9).
        assert_eq!(
            migrated["schema_version"], CURRENT_SCHEMA_VERSION,
            "migrated project must have schema_version == {CURRENT_SCHEMA_VERSION}"
        );

        // Assertion 2: every warp.zone_role is null.
        for (i, layer) in migrated["layers"].as_array().unwrap().iter().enumerate() {
            let zone_role = &layer["warp"]["zone_role"];
            assert!(
                zone_role.is_null(),
                "layer {i}: zone_role must be null after v7→v8 migration, got {zone_role}"
            );
        }

        // Assertion 3: deserialized project has zone_role = None on all layers.
        let project: Project =
            serde_json::from_value(migrated).expect("deserialize migrated project");
        for (i, layer) in project.layers.iter().enumerate() {
            assert_eq!(
                layer.warp.zone_role, None,
                "layer {i}: zone_role must be None in typed struct after migration"
            );
        }

        // Assertion 4: audit produces no zone-related findings.
        let findings = ProjectAudit::run(&project, &AuditEnv::default());
        let zone_findings: Vec<_> = findings
            .iter()
            .filter(|f| {
                matches!(
                    f.kind,
                    AuditKind::UnknownZoneRole { .. } | AuditKind::MissingZoneTag { .. }
                )
            })
            .collect();
        assert!(
            zone_findings.is_empty(),
            "v7 project with non-zone-consuming preset must produce no zone findings; got: {zone_findings:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // P6.2.3 — v8 → v9 migration tests
    // ---------------------------------------------------------------------------

    /// P6.2.3 — A v8 project saved with a `scenes` key migrates correctly:
    /// - `scenes` key is renamed to `cues`
    /// - Cue name and snapshot are preserved
    /// - Timing fields default correctly (in_time_s = 0.0, hold_time_s = null,
    ///   out_time_s = 0.0, fire_mode = GoOnTrigger, bpm_quantize = Off)
    #[test]
    fn migrate_v8_to_v9_scenes_renamed_to_cues() {
        use crate::project::schema::{BpmQuantize, CueFireMode, Project};

        let v8_json = serde_json::json!({
            "schema_version": 8,
            "layers": [],
            "scenes": [
                {
                    "name": "my intro scene",
                    "snapshot": {"layers": []},
                    "thumbnail": null
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        let (migrated, _) = migrate(v8_json).expect("migrate v8 → v10 (cascading)");

        // schema_version must be CURRENT_SCHEMA_VERSION (10 after P7.3.1).
        assert_eq!(
            migrated["schema_version"], CURRENT_SCHEMA_VERSION,
            "must be CURRENT_SCHEMA_VERSION"
        );

        // `scenes` key must be absent; `cues` key must be present.
        assert!(
            migrated.get("scenes").is_none(),
            "migrated output must not have `scenes` key"
        );
        assert!(
            migrated.get("cues").is_some(),
            "migrated output must have `cues` key"
        );

        // Deserialize and verify typed struct.
        let project: Project =
            serde_json::from_value(migrated).expect("deserialize migrated v9 project");

        assert_eq!(project.cues.len(), 1, "one cue must be present");
        assert_eq!(
            project.cues[0].name, "my intro scene",
            "cue name must be preserved"
        );
        // Timing fields must have identity defaults.
        assert_eq!(
            project.cues[0].in_time_s, 0.0,
            "in_time_s must default to 0.0"
        );
        assert_eq!(
            project.cues[0].hold_time_s, None,
            "hold_time_s must default to None"
        );
        assert_eq!(
            project.cues[0].out_time_s, 0.0,
            "out_time_s must default to 0.0"
        );
        assert_eq!(
            project.cues[0].fire_mode,
            CueFireMode::GoOnTrigger,
            "fire_mode must default to GoOnTrigger"
        );
        assert_eq!(
            project.cues[0].bpm_quantize,
            BpmQuantize::Off,
            "bpm_quantize must default to Off"
        );
        assert_eq!(
            project.cues[0].timecode_trigger, None,
            "timecode_trigger must default to None"
        );
        assert_eq!(
            project.cues[0].in_time_binding, None,
            "in_time_binding must default to None"
        );
    }

    /// P6.2.3 — A v9 project (native `cues` key) survives a save/reload
    /// round-trip with timing fields preserved.
    #[test]
    fn migrate_v9_cues_round_trip() {
        use crate::project::schema::{BpmQuantize, Cue, CueFireMode, Project};

        // Build a v9 project with non-default timing.
        let mut project = Project::default();
        let mut cue = Cue::new("cue-alpha", serde_json::json!({"layers": []}), None);
        cue.in_time_s = 2.5;
        cue.hold_time_s = Some(10.0);
        cue.out_time_s = 1.0;
        cue.fire_mode = CueFireMode::Follow;
        cue.bpm_quantize = BpmQuantize::Bars(4);
        project.cues.push(cue);

        // Serialize → migrate (should be a no-op for v9) → deserialize.
        let json_value = serde_json::to_value(&project).expect("serialize v9 project");
        let (migrated, _) = migrate(json_value).expect("migrate v9 project");
        let restored: Project = serde_json::from_value(migrated).expect("deserialize v9 project");

        assert_eq!(restored.cues.len(), 1, "one cue must survive round-trip");
        assert_eq!(restored.cues[0].name, "cue-alpha");
        assert!(
            (restored.cues[0].in_time_s - 2.5).abs() < 1e-6,
            "in_time_s must survive round-trip"
        );
        assert_eq!(restored.cues[0].hold_time_s, Some(10.0));
        assert!(
            (restored.cues[0].out_time_s - 1.0).abs() < 1e-6,
            "out_time_s must survive round-trip"
        );
        assert_eq!(restored.cues[0].fire_mode, CueFireMode::Follow);
        assert_eq!(restored.cues[0].bpm_quantize, BpmQuantize::Bars(4));
    }

    /// P7.3.1 — A v9 project with a per-layer `warp` field migrates to v10
    /// with `bezier_mesh: Some(...)` present on each layer.  The `anchors`
    /// array must equal the original `grid` verbatim.  All handles must be
    /// `None`.  The `mask_polygon`, `mask_feather`, and `zone_role` are
    /// preserved.
    #[test]
    fn migrate_v9_to_v10_adds_bezier_mesh() {
        use crate::project::schema::Project;

        let v9_json = serde_json::json!({
            "schema_version": 9,
            "layers": [
                {
                    "id": "layer-1",
                    "kind": {"Svg": {"svg_path": "test.svg"}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1,
                        "cols": 1,
                        "grid": [[[0.25, 0.25], [0.75, 0.25]], [[0.25, 0.75], [0.75, 0.75]]],
                        "mask_polygon": [[0.1, 0.2], [0.8, 0.2], [0.8, 0.8]],
                        "mask_feather": 0.05,
                        "zone_role": "window"
                    }
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        let (migrated, _) = migrate(v9_json).expect("migrate v9 → v11 (cascades through v10)");

        assert_eq!(
            migrated["schema_version"], CURRENT_SCHEMA_VERSION,
            "must be CURRENT_SCHEMA_VERSION (v10 → v11)"
        );

        let layer = &migrated["layers"][0];
        let bm = &layer["bezier_mesh"];
        assert!(!bm.is_null(), "bezier_mesh must be present after migration");

        // anchors == original grid
        assert_eq!(
            bm["anchors"], layer["warp"]["grid"],
            "anchors must equal original grid verbatim"
        );
        // rows / cols preserved
        assert_eq!(bm["rows"], 1_u64, "rows must be 1");
        assert_eq!(bm["cols"], 1_u64, "cols must be 1");
        // All handles_h are null (2×2 grid → 2×2 handle array)
        let handles_h = bm["handles_h"].as_array().expect("handles_h is array");
        assert_eq!(handles_h.len(), 2, "handles_h has rows+1 = 2 rows");
        for row in handles_h {
            for cell in row.as_array().expect("inner row is array") {
                assert!(cell.is_null(), "every handle_h entry must be null");
            }
        }
        // mask_polygon, mask_feather, zone_role preserved
        assert_eq!(
            bm["mask_polygon"],
            serde_json::json!([[0.1, 0.2], [0.8, 0.2], [0.8, 0.8]])
        );
        assert!((bm["mask_feather"].as_f64().unwrap() - 0.05).abs() < 1e-6);
        assert_eq!(bm["zone_role"], "window");

        // Full round-trip through typed struct
        let project: Project =
            serde_json::from_value(migrated).expect("deserialize migrated v10 project");
        let lc = &project.layers[0];
        let mesh = lc.bezier_mesh.as_ref().expect("bezier_mesh must be Some");
        assert_eq!(mesh.rows, 1);
        assert_eq!(mesh.cols, 1);
        assert_eq!(mesh.anchors.len(), 2); // rows+1
        assert_eq!(mesh.anchors[0].len(), 2); // cols+1
        assert!(
            mesh.handles_h.iter().flatten().all(|h| h.is_none()),
            "all handles_h must be None"
        );
        assert!(
            mesh.handles_v.iter().flatten().all(|h| h.is_none()),
            "all handles_v must be None"
        );
        // BezierMesh::from_warp_mesh / to_warp_mesh_lossless round-trip
        let warp_mesh = lc.warp.clone();
        let from_warp = crate::project::schema::BezierMesh::from_warp_mesh(&warp_mesh);
        let recovered = from_warp
            .to_warp_mesh_lossless()
            .expect("lossless round-trip");
        assert_eq!(
            warp_mesh.grid, recovered.grid,
            "from_warp_mesh → to_warp_mesh_lossless must be lossless for all-None handles"
        );
    }

    /// P7.4.1 — A v10 project with `bezier_mesh.mask_polygon` + `mask_feather`
    /// migrates to v11 with `mask_graph: Some(...)` containing a single Polygon
    /// node.  `mask_polygon` and `mask_feather` must be preserved in the Polygon
    /// node.
    #[test]
    fn migrate_v10_to_v11_adds_mask_graph() {
        use crate::project::schema::{MaskGraph, MaskNode, Project};

        // Build a v10 project with a warp that has a non-empty mask_polygon.
        let v10_json = serde_json::json!({
            "schema_version": 10,
            "layers": [
                {
                    "id": "layer-1",
                    "kind": {"Svg": {"svg_path": "test.svg"}},
                    "enabled": true,
                    "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
                    "effects": [],
                    "blend_mode": "Normal",
                    "opacity": 1.0,
                    "warp": {
                        "rows": 1,
                        "cols": 1,
                        "grid": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "mask_polygon": [[0.1, 0.2], [0.9, 0.2], [0.9, 0.8], [0.1, 0.8]],
                        "mask_feather": 0.03,
                        "zone_role": null
                    },
                    "bezier_mesh": {
                        "rows": 1,
                        "cols": 1,
                        "anchors": [[[0.0, 0.0], [1.0, 0.0]], [[0.0, 1.0], [1.0, 1.0]]],
                        "handles_h": [[null, null], [null, null]],
                        "handles_v": [[null, null], [null, null]],
                        "mask_polygon": [[0.1, 0.2], [0.9, 0.2], [0.9, 0.8], [0.1, 0.8]],
                        "mask_feather": 0.03,
                        "zone_role": null
                    }
                }
            ],
            "output_targets": [{"fallback_index": 0, "rgb_matrix": [[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]
        });

        let (migrated, _) = migrate(v10_json).expect("migrate v10 → v11");
        assert_eq!(
            migrated["schema_version"], CURRENT_SCHEMA_VERSION,
            "must be CURRENT_SCHEMA_VERSION (v11)"
        );

        let layer = &migrated["layers"][0];
        let mg = &layer["mask_graph"];
        assert!(!mg.is_null(), "mask_graph must be present after migration");

        let nodes = mg["nodes"].as_array().expect("nodes is array");
        assert_eq!(nodes.len(), 1, "single-node MaskGraph after migration");
        assert_eq!(nodes[0]["kind"], "Polygon", "node kind must be Polygon");
        assert_eq!(
            nodes[0]["points"],
            serde_json::json!([[0.1, 0.2], [0.9, 0.2], [0.9, 0.8], [0.1, 0.8]])
        );
        assert!((nodes[0]["feather"].as_f64().unwrap() - 0.03).abs() < 1e-6);

        // Full round-trip through typed struct.
        let project: Project =
            serde_json::from_value(migrated).expect("deserialize migrated v11 project");
        let lc = &project.layers[0];
        let mask_graph = lc.mask_graph.as_ref().expect("mask_graph must be Some");
        assert_eq!(mask_graph.nodes.len(), 1);
        if let MaskNode::Polygon { points, feather } = &mask_graph.nodes[0] {
            assert_eq!(points.len(), 4, "four polygon vertices");
            assert!((feather - 0.03).abs() < 1e-6);
        } else {
            panic!("expected Polygon node, got {:?}", mask_graph.nodes[0]);
        }

        // MaskGraph::from_polygon + identity check.
        let identity = MaskGraph::identity();
        assert_eq!(identity.nodes.len(), 1);
        if let MaskNode::Polygon { points, .. } = &identity.nodes[0] {
            assert!(points.is_empty(), "identity mask has empty polygon");
        } else {
            panic!("identity must have a Polygon node");
        }
    }

    // ---------------------------------------------------------------------------
    // 004-T1.4 — v11 → v12 migration tests
    // ---------------------------------------------------------------------------

    /// 004-T1.4 test 1 — a v11 layer with `treatment: {preset_id: "tone_map", …}`
    /// and two raw effects migrates to 3 EffectNodes, treatment first, all enabled.
    #[test]
    fn migrate_v11_to_v12_folds_treatment_in_front_of_effects() {
        let v11 = serde_json::json!({
            "schema_version": 11,
            "layers": [{
                "id": "a",
                "kind": { "Svg": { "svg_path": "/tmp/a.svg" } },
                "enabled": true,
                "transform": { "translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0] },
                "blend_mode": "Normal",
                "opacity": 1.0,
                // v11 effects are plain Effect objects (not EffectNode)
                "effects": [
                    { "Color": { "hue": { "Static": 0.0 }, "saturation": { "Static": 1.0 }, "brightness": { "Static": 0.0 }, "contrast": { "Static": 1.0 } } },
                    { "Blur": { "radius_px": { "Static": 0.0 } } }
                ],
                "treatment": {
                    "preset_id": "tone_map",
                    "params": { "exposure": 0.5 },
                    "overlay_path": null,
                    "collage_paths": []
                }
            }]
        });

        let (migrated, _) = migrate(v11).expect("migrate v11 → v12");
        let effects = &migrated["layers"][0]["effects"];
        let arr = effects.as_array().expect("effects must be an array");
        assert_eq!(arr.len(), 3, "3 nodes: Treatment + Color + Blur");

        // First node: Treatment
        let first = &arr[0];
        assert_eq!(first["enabled"], serde_json::json!(true), "enabled must be true");
        let treatment_variant = &first["effect"]["Treatment"];
        assert!(treatment_variant.is_object(), "first effect must be Treatment");
        assert_eq!(treatment_variant["id"], serde_json::json!("tone_map"));
        assert!((treatment_variant["params"]["exposure"].as_f64().unwrap() - 0.5).abs() < 1e-6);

        // Second and third nodes: Color and Blur — all enabled.
        assert_eq!(arr[1]["enabled"], serde_json::json!(true));
        assert!(arr[1]["effect"].get("Color").is_some(), "second must be Color");
        assert_eq!(arr[2]["enabled"], serde_json::json!(true));
        assert!(arr[2]["effect"].get("Blur").is_some(), "third must be Blur");

        // Full typed deserialise must succeed at v12.
        let _project: crate::project::Project =
            serde_json::from_value(migrated).expect("deserialize migrated v11");
    }

    /// 004-T1.4 test 2 — feeding a v12 value (already migrated) is idempotent:
    /// the output is byte-equal to the input (no double-wrap).
    #[test]
    fn migrate_v11_to_v12_idempotent_on_v12() {
        // Build a v12 project (post-migration shape): effects already have EffectNode form.
        let v12 = serde_json::json!({
            "schema_version": 12,
            "layers": [{
                "id": "b",
                "kind": { "Svg": { "svg_path": "/tmp/b.svg" } },
                "enabled": true,
                "transform": { "translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0] },
                "blend_mode": "Normal",
                "opacity": 1.0,
                "effects": [
                    { "enabled": true, "effect": { "Color": { "hue": { "Static": 0.0 }, "saturation": { "Static": 1.0 }, "brightness": { "Static": 0.0 }, "contrast": { "Static": 1.0 } } } }
                ]
            }]
        });

        let (migrated, _) = migrate(v12.clone()).expect("migrate v12");
        // schema_version is updated to CURRENT (also 12) — that's fine.
        // The effects array must be unchanged: still 1 node, not double-wrapped.
        let effects = &migrated["layers"][0]["effects"];
        let arr = effects.as_array().expect("effects must be array");
        assert_eq!(arr.len(), 1, "idempotent: still 1 node after re-migration");
        assert_eq!(arr[0]["enabled"], serde_json::json!(true));
        assert!(arr[0]["effect"].get("Color").is_some(), "Color node preserved");
    }

    /// 004-T1.4 test 3 — treatment with `overlay_path` pointing to a non-existent
    /// file migrates without reading the filesystem: the path is carried verbatim.
    #[test]
    fn migrate_v11_to_v12_byte_for_byte_with_missing_overlay() {
        let nonexistent = "/nonexistent/does/not/exist/overlay.png";
        let v11 = serde_json::json!({
            "schema_version": 11,
            "layers": [{
                "id": "c",
                "kind": { "Svg": { "svg_path": "/tmp/c.svg" } },
                "enabled": true,
                "transform": { "translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0] },
                "blend_mode": "Normal",
                "opacity": 1.0,
                "effects": [],
                "treatment": {
                    "preset_id": "texture_overlay",
                    "params": {},
                    "overlay_path": nonexistent,
                    "collage_paths": []
                }
            }]
        });

        // Must not panic, must not try to open the file.
        let (migrated, _) = migrate(v11).expect("migrate v11 with missing overlay — must succeed");
        let effects = &migrated["layers"][0]["effects"];
        let arr = effects.as_array().expect("effects must be array");
        assert_eq!(arr.len(), 1, "one EffectNode (the Treatment)");
        let treatment = &arr[0]["effect"]["Treatment"];
        assert_eq!(treatment["id"], serde_json::json!("texture_overlay"));
        // overlay_path carried verbatim — no filesystem access.
        assert_eq!(
            treatment["overlay_path"].as_str().unwrap(),
            nonexistent,
            "overlay_path must be preserved byte-for-byte"
        );
    }
}
