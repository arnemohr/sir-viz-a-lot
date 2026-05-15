# 004 — Treatment Overhaul: Unified "Look Chain" per Layer

## Context

`rmap`'s current model exposes two parallel concepts per layer: `LayerConfig.treatment: Option<Treatment>` (one slot, full SDF/zone/particle plumbing) and `LayerConfig.effects: Vec<Effect>` (a chain, but the `Effect::Treatment` variant **silently drops** SDF/zone/seed/overlay/collage — see `src/effects/mod.rs:295-345`). The result is three stacked invisible-failure modes:

1. Empty `warp.mask_polygon` ⇒ every SDF-keyed treatment is a passthrough (root cause of the bug that triggered this spec — operator added `ripple_lens` to a video layer and saw nothing).
2. The same preset placed in `Effect::Treatment` is a passthrough for a different reason (no SDF plumbing in `RenderCtx`).
3. UI is split across two surfaces (Treatment CollapsingHeader in Layers tab, dedicated Effects tab) so operators don't see the chain as one thing.

Anchored to Chesky's 11-star ladder, the design target is the 7–8 star tier: **a single ordered Look chain per layer with status dots, headline-param-on-row, bypass per node, one-tap autofix chips, and intent-grouped Add picker**. No silent failures. No "treatment vs effect" vocabulary leaking to the operator. The word "treatment" disappears from the UI; operators see nodes grouped by intent (Warp / Color / Texture / Compose / Animate / Generative).

Decisions locked in: **(a) full schema unification at v12** (no half-migration), and **(b) live preset thumbnails ship Phase 2**, not Phase 1.

This spec executes Phase 1 in 2–3 engineering weeks.

---

## A. Schema + render-graph foundation (v11→v12)

### A.1 New `EffectNode` wrapper

In `src/effects/mod.rs` (next to `Effect` at line 17):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectNode {
    #[serde(default = "default_enabled_true")]
    pub enabled: bool,
    pub effect: Effect,
}
fn default_enabled_true() -> bool { true }
```

**`default_enabled_true` is load-bearing.** Plain `#[serde(default)]` evaluates `bool` to `false`, which would silently bypass every effect on any pre-v12 save and every `assets/presets/*.json` (sampled — they're flat `Effect` arrays).

In `src/project/schema.rs:243-287`:
- Drop `pub treatment: Option<Treatment>` (line 271).
- Change `pub effects: Vec<Effect>` (line 248) → `pub effects: Vec<EffectNode>`.
- Keep the `Treatment` struct definition (used by the migrator); just no longer a `LayerConfig` field.

### A.2 `Effect::Treatment` gains overlay/collage paths

In `src/effects/mod.rs:69-73`:

```rust
Treatment {
    id: String,
    #[serde(default)] params: HashMap<String, f32>,
    #[serde(default)] overlay_path: Option<PathBuf>,
    #[serde(default)] collage_paths: Vec<PathBuf>,
},
```

Keeps overlay/collage as data-of-the-treatment, not data-of-the-layer.

### A.3 Migration step

`src/project/schema.rs:10`: `CURRENT_SCHEMA_VERSION` 11 → 12.

`src/project/migrate.rs`: extend the `0..=10` arm at line 48 to `0..=11`; add:

```rust
if version <= 11 {
    migrate_v11_to_v12_fold_treatment_into_effects(&mut value);
}
```

Body (operates on `serde_json::Value`, same pattern as the existing migrators at lines 169–200):

- For each `layer` in `value["layers"]`:
  - Read `effects` (default `[]`) and `treatment` (default `null`).
  - Build new `Vec<Value>`:
    - If `treatment` is an object with `preset_id`: prepend `{"enabled": true, "effect": {"Treatment": {"id": …, "params": …, "overlay_path": …, "collage_paths": […]}}}`.
    - Wrap each existing effect as `{"enabled": true, "effect": <existing>}`.
  - Replace `layer["effects"]`; remove `layer["treatment"]`.
- Skip empty/null `preset_id` (matches existing v11 audit which warns on these).

**Tests:**
- `migrate_v11_to_v12_folds_treatment_in_front_of_effects` — fixture with `treatment.preset_id = "tone_map"` + 2 effects → assert `effects.len() == 3`, first id `"tone_map"`, all `enabled: true`.
- Idempotency test: re-running on v12 input is a no-op.
- Missing `overlay_path` file migrates byte-for-byte (audit fires on load, migration is disk-blind).

### A.4 Render-graph rewiring

**`RenderCtx` adds 6 fields** in `src/effects/mod.rs:101-147`:

```rust
pub sdf_view: Option<&'a wgpu::TextureView>,
pub zone_role: Option<crate::project::schema::ZoneRole>,
pub seed: u64,
pub t_layer_added_secs: f32,
pub overlay_view: Option<&'a wgpu::TextureView>,
pub collage_views: &'a [&'a wgpu::TextureView],
```

**`Effect::Treatment` arm in `src/effects/mod.rs:295-345`**: replace the hardcoded `sdf: None, zone_role: None, seed: 0, ...` with `ctx.sdf_view`, `ctx.zone_role`, etc.

**Caller-side at `src/app.rs:4429-4593`**:
- **Delete** the primary-treatment dispatch (lines 4429-4559). Treatment is now just the first `Effect::Treatment` in `effects` and runs through the same loop. `svg_pipeline.render` becomes unconditional (it's a passthrough blit; visually identical because the next pass — `Effect::Treatment` — reads what `svg_pipeline` wrote).
- **Hoist** overlay/collage loading from the deleted block into the per-node loop, dispatched only when the node's effect is `Effect::Treatment` with paths. `image_texture_cache.lookup_or_upload` is Arc-counted; per-node cost is one HashMap lookup.
- **Sync SDF once** before the loop (`ls.warp_renderer.sync_from_layer`), reuse `ls.warp_renderer.sdf_view()` for every node.
- **Per-node bypass:** `for node in &cfg.effects { if !node.enabled || st.ab_compare { continue; } … }`. Skipping does NOT flip the ping-pong, so the upstream pixels passthrough automatically.
- Thread `ls.layer_id.0` as `seed`, `cfg.warp.zone_role` as `zone_role`, `0.0` as `t_layer_added_secs` (Image/Video convention).

### A.5 Mutation cleanup

In `src/project/command.rs`:
- **Delete** `SetLayerTreatment` (lines 905-941), `SetLayerTreatmentParams` (943-987), their `Mutation` enum variants (~2418, ~2423), match arms (~2603-2606), undoability entries (~2808-2809), constructors `set_layer_treatment_mutation` (~2947) and `set_layer_treatment_params_mutation` (~2963).
- **Delete** the corresponding round-trip tests (~3812-3925) and proptest enumerator entries (~5131, 5137, 5577, 5595, 5940, 5942).
- **Update** `SetLayerEffects` (line 743) to carry `Vec<EffectNode>`. `ReverseStorage` impl structurally unchanged. The "Effects-Vec Reverse" rule (project/CLAUDE.md) still holds — whole-vec snapshot.
- **Update** `ModulatorField` and `modulator_at_{ref,mut}` (command.rs:71-146) to dereference `EffectNode.effect`.

### A.6 Auxiliary call sites

- `src/windows/control_panel.rs:1550, 1602, 1689, 2275-2280, 2588`: `effects[idx]` reads dereference `.effect`. Touched anyway by the UI rewrite in C.
- `src/windows/control_panel.rs:236-238` (`Preset.effects: Vec<Effect>`): **keep as-is**. When applied (line 1547-1559), wrap each Effect in `EffectNode { enabled: true, effect: e }` at construction. Avoids migrating `assets/presets/*.json` (sampled — currently flat `Effect` arrays).
- `src/windows/scene_editor.rs::mutate_transform_effect` appends a default `Effect::Transform` if absent — must now append `EffectNode { enabled: true, effect: Effect::Transform {…} }`. The Reverse-storage rule (project/CLAUDE.md rule 2) doesn't change.
- `src/project/scene_instantiation.rs` / `scene_templates.rs`: confirm no `.treatment` references (grep returned none); ensure new `LayerConfig` literals use `effects: vec![]`.

### A.7 `Project::interpolate` and crossfade

`EffectNode.enabled: bool` is categorical; under existing `interpolate` semantics it snaps at `t=0.5` (same as `BlendMode::Normal → Add`). Document but don't add fade semantics in Phase 1.

---

## B. Capability metadata + no-op detection

### B.1 `PresetCapability` (new)

In `src/render/treatments.rs` near `ParamDescriptor` (~line 251):

```rust
pub struct PresetCapability {
    pub requires_sdf: bool,
    pub requires_zone: bool,
    pub is_particle: bool,
    pub headline_param: Option<&'static str>,
}
pub fn capability(preset_id: &str) -> PresetCapability { … }
```

Populate from existing W2 comments and `treatment_group` (treatments.rs:289). `requires_sdf=true` for `ripple_lens`, `displacement_ripple`, `edge_lens`, `field_advect`, `refraction`, `blur_mask`, `fluid_warp`, `zone_brighten`, `zone_lens`, `spotlights`, all `drift_*`, `edge_sparks`, `collision_ripples`, `portal_warp`. `requires_zone=true` for the two `zone_*`. `is_particle=true` for the particle siblings. `headline_param` = `Some("amplitude")` for ripple_lens, `Some("radius_px")` for blur_mask, `Some("intensity")` for tone_map, etc.

### B.2 No-op detection

In `treatments.rs`:

```rust
pub fn treatment_is_no_op(preset_id: &str, params: &HashMap<String, f32>, layer: &LayerConfig)
    -> Option<&'static str>
```

Returns:
- `requires_sdf(preset) && layer.warp.mask_polygon.is_empty()` → `"Needs a mask polygon"`
- `requires_zone(preset) && layer.warp.zone_role.is_none()` → `"Needs a zone role"`
- `ripple_lens` with `amplitude ≈ 0` → `"Amplitude at 0"`
- `tone_map` at identity → `"All params at identity"`
- `texture_overlay` with no path → `"Overlay file missing"`

Sibling `effect_is_no_op(&EffectNode) -> Option<&'static str>` in `src/effects/mod.rs` for the non-treatment variants (Color identity, Tint amount=0, Blur radius=0, Transform identity, Feedback decay=0).

### B.3 Intent groups

Add `pub fn intent_group(&Effect) -> IntentGroup` in `src/effects/mod.rs` mapping every variant + every treatment preset_id to one of six groups: `Warp | Color | Texture | Compose | Animate | Generative`. Drives picker grouping and row glyph color.

---

## C. Unified UI: `show_look_chain_section`

### C.1 New module

Create `src/windows/look_chain.rs`. Public entry `pub fn show_look_chain_section(ui, project, st, layer_idx)`. Register in `src/windows/mod.rs`.

### C.2 Splice points

- `src/windows/controls.rs:339`: replace `show_treatment_section(...)` with `look_chain::show_look_chain_section(...)`. Rename the surrounding `CollapsingHeader` from `"Treatment"` to `"Look chain"`, default-open `true`.
- **Delete** `show_treatment_section` (controls.rs:857-1013) entirely.
- **Delete the Effects tab**: `ControlTab::Effects` enum variant, the selectable_value at `control_panel.rs:586`, the match arm at line 918, `show_effects_tab` (line 1494), and the helper `add_effect_picker` it inlines.
- Preset apply path (control_panel.rs:1547-1559): fold into the Look chain section as a small "Preset" sub-control above the chain rail.

### C.3 Row anatomy

For each `EffectNode` in `cfg.effects`, render one row inside `ui.dnd_drop_zone::<usize, _>` (mirror `control_panel.rs:1596-1656` verbatim):

- **Drag handle:** `egui::Label::new("⋮⋮").sense(Sense::drag())` with `dnd_set_drag_payload(idx)`.
- **Preset glyph:** colored `RichText` glyph keyed by `intent_group` (Warp 🌀, Color 🎨, Texture 🧱, Compose 🧩, Animate 🌊, Generative ✨). Color from `egui::Color32` constants per group — six-color taxonomy doubles as visual signature.
- **Status dot:** 10px circle painted via `ui.painter()`. Green = `is_no_op` returned `None`. Amber = `Some(reason)`. Grey = `!node.enabled`. Click toggles `enabled` (dispatches `SetLayerEffects` snapshot).
- **Headline param on-row:** look up `primary_param(preset_id)` for `Effect::Treatment`; fixed per-variant choice for the others (Color → brightness, Blur → radius_px, Tint → amount, Transform → scale_x, Feedback → decay). Use `modulator_slider` (control_panel.rs:2695-2776) for `Modulator`-typed slots; plain `egui::Slider::new(&mut f32, min..=max)` for `Treatment.params` HashMap slots. **Decision:** no fake modulator-bind affordances on treatment params — Phase 3 lifts treatment params into `Modulator` and unifies.
- **Expand chevron:** `CollapsingHeader` with `id_salt((layer_idx, idx))`. When open, render full param list (reuse the per-param loop from `controls.rs:993-1009` for treatments; existing per-effect slider blocks for non-treatment variants).
- **Delete:** `×` matching `control_panel.rs:1622-1628`. Snapshot mutation removes the node.

Staged drag/remove/edit changes apply to a clone of `cfg.effects` after the loop, then dispatch as a single `SetLayerEffects` — same pattern as `control_panel.rs:1643-1672`.

### C.4 Add picker

Replace the combobox at `control_panel.rs:1679`. New is a button `[ + Add to chain ▾ ]` opening an `egui::Popup` with intent-grouped sections (one collapsing block per `IntentGroup`). Inside each block, `selectable_label` rows: every `Effect` variant under its group, every `Effect::Treatment` preset under its group (from `treatments::registry()` at line 317).

Each row carries:
- Group glyph + name.
- `on_hover_text` with description + capability hints ("Needs a mask polygon", "Needs a zone role").
- Click handler: build `EffectNode { enabled: true, effect: <variant with identity-default params> }`, append to cloned `cfg.effects`, dispatch `SetLayerEffects`. Smart-fill bundling (D).

### C.5 Autofix chips

When `is_no_op` returns `Some(reason)`, render inline `egui::Button::new(format!("⚠ {reason} — [auto-fix]")).small()` next to the status dot. Click dispatches a specific `Mutation`:
- `"Needs a mask polygon"` → `SetLayerMaskPolygon { new: full_layer_quad, old: empty }` (constructor exists at `command.rs` alongside mask-vertex mutations).
- `"Amplitude at 0"` / `"Intensity at 0"` → `SetLayerEffects` snapshot with `params.insert(headline_param, identity_nudge)` (e.g., amplitude → 0.3) applied to the specific node.
- `"Overlay file missing"` → file dialog, then `SetLayerEffects` snapshot replacing the node's `Effect::Treatment.overlay_path`.

Each chip is a button; no new types.

### C.6 A/B compare

Add `pub ab_compare: bool` to `ControlPanelState` (default `false`). Look-chain header gets a `[ A/B compare ]` toggle. **Not** in the project snapshot — does not crossfade, does not undo, does not persist. Render-loop hook is the single check at `src/app.rs:4561`: `if ab_compare || !node.enabled { continue; }`. Thread `ab_compare` through the per-frame render call alongside other session flags.

---

## D. Smart-fill on add ("drunk-proof" finishing touch)

In the Add-picker click handler (C.4): if the user adds an SDF-keyed preset to a layer with empty `warp.mask_polygon`, the same mutation transaction also adds a full-layer-quad mask polygon. **Single undo step.**

Implementation: new `Mutation::AddEffectNodeWithMaskFill { layer_idx, new_node, new_mask_polygon, old_effects, old_mask_polygon }` in `src/project/command.rs` with `ReverseStorage` impl snapshotting both `effects` and `warp.mask_polygon` (combines Reverse rules 1 + 2). Dispatched only from the Add-picker. Same shape extended for `needs_zone && zone_role.is_none()` → defaults to `ZoneRole::Window`.

Alternative considered: two separate mutations pushed back-to-back. Rejected because v3's undo treats each mutation as a separate undo step — the operator would have to press Cmd-Z twice for what felt like one action.

---

## E. Phasing + estimates

- **Phase 1 (ship — 2-3 weeks):** A, B, C, D. Schema + render-graph + capability metadata + Look chain UI + autofix chips + A/B compare + smart-fill. The full unified experience minus thumbnails.
- **Phase 2 (1-2 weeks):** Live preset thumbnails in the picker — 36 nodes at 256×256, ~1-2 ms/frame on Apple silicon. Render once per preset id into a cache texture; egui draws via `register_native_texture`. No schema changes.
- **Phase 3 (1 week):** Treatment params upgrade to `HashMap<String, Modulator>` so audio/MIDI/BPM binding works uniformly across all chain nodes. Schema bump v12→v13, per-param `Static(f32)` wrap migrator.
- **Phase 4 (aspirational):** Stage-thumbnail strip ("solo up to here"), vibe presets (curated chain templates), smart suggestions ("this layer would benefit from edge_sparks"). Outside this spec.

---

## F. Risks + open questions

1. **`enabled: bool` defaults to `false`.** The `default_enabled_true` helper in A.1 is the single highest-impact line of code. Without it, every pre-v12 save and every `assets/presets/*.json` loads with all effects bypassed. Add a unit test that deserializes `{"effect": {"Color": {…}}}` (no `enabled` key) and asserts `node.enabled == true`.
2. **v12 migration with missing overlay file.** Migration is disk-blind. Existing `AuditKind::MissingAsset` (audit.rs:577, 602) fires on load — no regression.
3. **Audit findings address change.** `audit.rs:535-590` iterates `layer.treatment.as_ref()`. Post-v12 it must iterate `layer.effects` with per-node match on `Effect::Treatment`. `AuditKind::UnknownTreatment { layer_idx, preset_id }` extends to `{ layer_idx, effect_idx, preset_id }` so the chip can highlight the specific row. Same for two `MissingAsset` findings at 577, 602.
4. **Audit tests that hardcode `layer.treatment = Some(...)`.** Confirmed at `audit.rs:1166, 1194, 1231, 1756, 1782, 1803`. Rewrite to push `EffectNode { enabled: true, effect: Effect::Treatment {…} }` into `effects`.
5. **`command.rs` treatment-mutation tests.** Deleted with the mutations (lines 3812-3925).
6. **Render-graph perf.** Treatment now runs as the first chain pass instead of its own dedicated dispatch. Net cost: one extra ping-pong flip per layer with a treatment. < 50µs per layer on Apple silicon. No frame-budget assumption breaks.
7. **`Preset.effects` JSONs in `assets/presets/`.** Sample (`architectural_wash.json`) is flat `Effect` arrays. Keep `Preset.effects: Vec<Effect>` and wrap on apply (C.2). Zero JSON migration.
8. **`interpolate` snap-cut on `enabled` toggle.** Same as current `BlendMode` categorical behavior — document, defer fade-via-bypass to Phase 4.
9. **Operators with Effects-tab muscle memory.** One-shot toast on first launch ("Effects merged into the Layers tab as Look chain") gated on a session flag in `~/Library/Application Support/`.

---

## Critical files to modify

| File | Phase 1 change |
| --- | --- |
| `src/effects/mod.rs` | Add `EffectNode` (A.1), extend `Effect::Treatment` (A.2), add 6 fields to `RenderCtx` (A.4), wire ctx through `Effect::Treatment` dispatch, add `effect_is_no_op`, `intent_group` (B.2-3) |
| `src/project/schema.rs` | Drop `LayerConfig.treatment`, change `effects` to `Vec<EffectNode>`, bump `CURRENT_SCHEMA_VERSION` to 12 (A.1, A.3) |
| `src/project/migrate.rs` | Add `migrate_v11_to_v12_fold_treatment_into_effects` (A.3) |
| `src/project/command.rs` | Delete `SetLayerTreatment{,Params}` + constructors + tests; update `SetLayerEffects` to `Vec<EffectNode>`; update `ModulatorField` accessors; add `AddEffectNodeWithMaskFill` (A.5, D) |
| `src/project/audit.rs` | Iterate `effects` instead of `treatment`; extend `UnknownTreatment`/`MissingAsset` kinds with `effect_idx`; update tests (F.3-4) |
| `src/render/treatments.rs` | Add `PresetCapability`, `capability`, `treatment_is_no_op`, `primary_param`, `requires_sdf`, `intent_group` (B.1-3) |
| `src/app.rs` | Delete primary-treatment dispatch at 4429-4559; thread 6 new ctx fields; per-node bypass + A/B-compare check at 4561 (A.4, C.6) |
| `src/windows/look_chain.rs` | **New.** `show_look_chain_section` — row anatomy, drag-reorder, headline slider, status dot + autofix chip, expand panel, A/B-compare toggle (C.1-6) |
| `src/windows/controls.rs` | Replace splice at line 339; delete `show_treatment_section` 857-1013 (C.2) |
| `src/windows/control_panel.rs` | Remove `ControlTab::Effects`, `show_effects_tab` 1494+, `add_effect_picker`; wrap `Preset.effects` on apply at 1547-1559 (A.6, C.2) |
| `src/windows/scene_editor.rs` | `mutate_transform_effect` appends `EffectNode` wrapper (A.6) |

## Reused functions / utilities (do not reinvent)

- `dnd_drop_zone::<usize, _>` drag-reorder pattern — `src/windows/control_panel.rs:1596-1656`
- `modulator_slider(ui, salt, label, m, range, field, effect_idx, layer_idx)` — `src/windows/control_panel.rs:2695-2776`
- Per-param slider loop pattern — `src/windows/controls.rs:993-1009`
- `ImageTextureCache::lookup_or_upload` — `src/image_layer/`
- `WarpRenderer::sdf_view()` / `sync_from_layer` — `src/render/warp.rs:518, 618`
- `treatments::registry()`, `treatment_group` — `src/render/treatments.rs:317, 289`
- `param_descriptors` — `src/render/treatments.rs:383-422`
- `set_layer_effects_mutation` constructor — `src/project/command.rs:739`
- `SetLayerMaskPolygon` mutation — exists, used by the empty-mask audit autofix
- `AuditFinding.autofix: Option<Mutation>` — `src/project/audit.rs:244` (parallel pattern; toast-click wiring stays deferred)

## Verification

End-to-end manual test (must pass before declaring Phase 1 done):

1. **Migration smoke.** Open a v11 fixture project containing one layer with `treatment: { preset_id: "ripple_lens", params: { amplitude: 0.05 } }` plus two effects. After load, `project.layers[0].effects.len() == 3`, first node is the ripple_lens, all `enabled: true`. Save → reload → identical.
2. **SDF-keyed treatment works anywhere.** Add `ripple_lens` to chain position 0 → wiggle visible. Move via drag to position 2 (after a `Blur` and a `Tint`) → still wiggles. (This is the canary for the SDF/zone/seed plumbing fix in A.4.)
3. **No-mask autofix.** New Video layer (default `mask_polygon: []`). Add `ripple_lens`. With smart-fill: ripples appear immediately, undo restores empty mask AND removes the node in one step. Without smart-fill flag set: amber status dot, "Needs a mask polygon — [auto-fix]" chip; click chip → ripples appear.
4. **Bypass per node.** Click status dot on a node → grey, output ignores that node. Click again → restored. Cmd-Z reverses each toggle as one step.
5. **A/B compare.** Toggle the header button → all nodes bypassed, raw source shown. Toggle off → restored. Cmd-Z does NOT touch A/B state (session-only flag).
6. **Drag-reorder.** Drag ripple_lens past blur. Visual updates immediately. One undo step.
7. **Picker grouping.** Click `+ Add to chain ▾`. Verify intent groups (Warp, Color, Texture, Compose, Animate, Generative) — every preset appears under exactly one, no preset orphaned, "treatment" string nowhere in operator-facing copy.
8. **Render perf.** With a 4-node chain on each of 8 layers at 4K, frame time within 2% of v11 baseline (one extra ping-pong flip per layer with a former-treatment, < 50µs/layer estimate).

Test suites:
- `make test` (nextest) — adds passing for new migrator test, new no-op detection tests, updated audit tests.
- `make test-gpu` — golden images for the per-layer effect chain re-baseline once (ripple_lens at chain position 0 produces the same output as v11 ripple_lens in the primary slot; golden delta should be zero modulo blit precision).
- `make ci` — fmt + clippy + doctests + nextest. Doctest on `EffectNode` serde defaults validates the `default_enabled_true` invariant.

Cross-check the `default_enabled_true` invariant on every preset JSON in `assets/presets/` after migration: load each, assert all wrapped nodes have `enabled: true`.
