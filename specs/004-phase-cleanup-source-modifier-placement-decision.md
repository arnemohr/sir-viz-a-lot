# Decision: where SourceModifier semantics live in the render graph

**Status:** Decided — Option B (Treatment). Decision drives PCleanup.1.3 and re-paths PCleanup.1.2 + W2.1–W2.11.
**Affects:** PCleanup.1.2 (`fluid_warp` pipeline placement), PCleanup.1.3 (`Effect::Treatment` variant), PCleanup.2.1–PCleanup.2.11 (12 sibling SourceModifier presets), PCleanup.1.1 (the `FxFamily::SourceModifier` variant is preserved but de-prioritised).

---

## Background

PCleanup.1.1 added `FxFamily::SourceModifier` to `FxFamily` in `src/render/fx_presets.rs` on the assumption that source-modifying presets would dispatch through the FX preset registry, alongside `Fragment`, `ComputeParticle`, and `ComputeFluid` variants. The spec for PCleanup.1.2 called for `fluid_warp` to register under that family.

During implementation, a structural mismatch surfaced:

- **FX presets dispatch only on FxLayer** (`src/render/fx_presets.rs::dispatch(preset_id, pipelines, inputs)`, called from `src/app.rs:4127`). FxLayer is procedural — it has no inherent source texture. The dispatch call site hardcodes `source: None`.
- **SourceModifier semantics — warp the underlying photo — require a real source.** That source naturally lives on Image, Video, or SVG layers, which run their pixels through the `Effect` chain (`src/effects/`) and the `Treatment` pipeline (`src/render/treatments.rs`), not the FX preset registry.

So `fluid_warp` as an FX preset has no source to bind. The `inputs.source: Option<&TextureView>` field on `FxShaderInputs` (added by P2.3.2 with `#[allow(dead_code)]` and the comment "wired by future Wave/Fluid families that composite over source") is plumbed but never populated, because the only consumer (FxLayer dispatch) doesn't have a meaningful source to put there.

This blocks PCleanup.1.2 in its spec'd form and, by extension, the 12 sibling presets in W2.1–W2.11.

---

## Options

### Option A — FxLayer becomes a passthrough that consumes the layer below

Change FxLayer semantics so it reads the accumulator-so-far (the Compositor's output up to that point in the layer stack) as its source. FxLayer at index N modifies the visual output of layers 0..N-1, then writes the modified result back into the accumulator.

**Pros:**
- The most natural rendering of "adjustment layer" semantics (cf. Photoshop, After Effects). An operator builds a stack of Image/Video layers, then drops a `fluid_warp` FxLayer on top — the layers below distort as the fluid flows.
- Architecturally correct long-term — operators expect adjustment-layer semantics from any non-trivial compositor.
- Reuses the FX preset registry as the home for source-modifying *and* generative work; one tier for all per-layer GPU passes.

**Cons:**
- Substantial render-graph change. The Compositor currently blends each layer's pre-warp output independently; an accumulator-aware FxLayer needs the in-progress compositor texture threaded into dispatch. Two passes per FxLayer: composite up to N, then run the FX, then continue. Likely a week+ of work and several rounds of golden-image regression testing.
- Risk of cycles when an FxLayer's mask depends on what's below (the mask SDF is layer-local, but the source is global).
- Operator UX shift: FxLayer changes from "self-contained procedural layer" to "depends on stacking order." Worth a separate UX decision.
- Out of scope for the cleanup phase. Cleanup is meant to convert stranded features into shipped capability, not redesign rendering primitives.

### Option B — Reposition `fluid_warp` (and W2 siblings) as Treatments

Drop the SourceModifier semantics from the FX preset registry. Instead, ship PCleanup.1.3 (`Effect::Treatment(id, params)`) and add `fluid_warp` as Treatment #10 (alongside the existing 9). The same applies to all 12 W2 sibling presets — they become treatments, not FX presets.

**Pros:**
- **The plumbing already exists.** `TreatmentInputs::source: &TextureView` is the first bind on every treatment dispatch (`src/render/treatments.rs:436`, `src/app.rs:4327`). Treatments are precisely the source-reading per-layer passes the SourceModifier concept needed.
- **Two of the existing treatments** — `displacement_ripple` and `refraction` — are already source-modifying effects matching the SourceModifier intent. Adding `fluid_warp` and the W2 siblings extends a path that's already real and tested.
- **Cleanest mental model.** Treatments = source-reading per-layer passes (modulators of the photo). FX presets = generative procedural layers (creators of new pixels). Operators can learn one rule: "warp the photo? Treatment. Make a particle swarm? FX preset." Currently the rule is fuzzy because both tiers are labelled as "effects" in the UI.
- **Smallest blast radius for the cleanup phase.** PCleanup.1.3 was already planned and prioritised; making it the home for SourceModifier semantics raises its leverage without expanding scope.
- **Doesn't paint us into a corner for Option A.** If a future phase decides to ship adjustment-layer semantics (a worthwhile thing to consider for v2.0), nothing in Option B blocks it. The FX preset registry just grows fewer "presets that don't really belong here."

**Cons:**
- `FxFamily::SourceModifier` (already added by PCleanup.1.1) becomes a reserved variant with no immediate use. Acceptable — keep it with an updated doc comment as forward-compat for the possible Option A migration, or retire it cleanly. Either is fine.
- The `fx_fluid_warp.wgsl` shader (already committed by partial PCleanup.1.2) gets adopted by the Treatment dispatcher instead of the FX preset dispatcher. Same shader file, different pipeline harness.
- Treatments today are mostly fragment passes; `fluid_warp` (and a few W2 siblings) need a compute prelude for their velocity field. The `TreatmentPipeline` may need a small extension to own per-instance compute state. Manageable.

### Option C — New `Effect::FxPreset(id, params)` variant

Add a new `Effect` variant that dispatches into the FX preset registry but runs on a real layer's source. This gives FX presets a second invocation path: the existing FxLayer dispatch (no source) plus the new Effect-chain dispatch (with source from Image/Video/SVG).

**Pros:**
- Preserves the FX preset registry as the home for all `fluid_warp`-style work. No re-pathing of W2.1–W2.11.
- Lets the same preset shader run in two contexts. In theory, more flexibility.

**Cons:**
- **Two dispatch paths for the same preset.** The same `fluid_warp` ID would resolve differently depending on whether the operator added it to an FxLayer (no source) or an Image layer's Effect chain (with source). Confusing operator UX: where does fluid_warp live in the UI? FX panel or Effect chain? Twice the maintenance: every preset shader either needs `source: Option<TextureView>` and branch on it, or has two pipeline variants, or works in only one context.
- **Worst of both worlds for the mental model.** Treatments stay as a separate tier, AND FX presets get a source-aware path. Three tiers (Effect chain, Treatment chain, FX preset-via-Effect) with overlapping semantics. The opposite of cleanup.
- More code surface than Option B for less behavioural value.

---

## Decision

**Option B: Treatments.**

Routing:
- **PCleanup.1.3 ships first.** Add `Effect::Treatment(id, params)` that dispatches into the existing `TreatmentPipeline` per-layer. Initial scope: the trivial-per-layer treatments (`identity`, `tone_map`, `luminance_reveal`, `blur_mask`, `palette_extract`, `displacement_ripple`). Treatments that need external assets (`texture_overlay`, `collage`, `refraction`) need additional asset-management plumbing and are deferred to a follow-up.
- **`fluid_warp` becomes Treatment #10** in a follow-up commit. The existing `fx_fluid_warp.wgsl` shader is the right shader; it gets adopted by the Treatment dispatcher. The compute prelude (velocity advection) lives in the `TreatmentPipeline` instance for `fluid_warp` — `TreatmentPipeline` gains the ability to own per-instance compute state. The work that landed in partial PCleanup.1.2 (the shader + the design surfacing) carries forward in full.
- **W2.1–W2.11 sibling presets land as treatments**, not FX presets. Most (ripple_lens, edge_lens, spotlights, drift_pinholes, edge_sparks, zone_brighten, zone_lens) are pure fragment passes — they fit the existing Treatment shape directly, no compute extension required.
- **`FxFamily::SourceModifier` stays as a reserved variant.** Doc comment updates to: "Reserved for future compute-attached treatments and a possible Option-A adjustment-layer migration. Primary source-modifying path is `Effect::Treatment` per PCleanup.1.3." Tests `no_source_modifier_presets_yet_registered` and `source_modifier_variant_is_distinct` continue to pass.
- **Glossary entries (PCleanup.0.1)** stay verbatim. The operator-facing capability names (`fluid_warp`, `ripple_lens`, …) don't change; only the pipeline tier underneath does.
- **Effect chain UI** in `src/windows/controls.rs` gets the new `Effect::Treatment(id)` variant. The treatment-picker dropdown surface for per-layer treatments lands as part of PCleanup.1.3 work or a small follow-up.

---

## Implications

- **PCleanup.1.2 acceptance criteria re-read against Option B.** The shader is shipped (✅ commit 2a30578). The pipeline + dispatch wiring moves to Treatment land, not FX-preset land. Mark PCleanup.1.2 "complete (re-pathed; full implementation absorbed into PCleanup.1.3 follow-up)."
- **PCleanup.1.1 acceptance criteria still met.** `FxFamily::SourceModifier` variant exists, dispatches exhaustively, no presets register against it. The reserved-variant status is intentional after this decision.
- **Spec file (`004-phase-cleanup.md`) needs a one-paragraph addendum** pointing to this decision doc and noting that W2.* tasks land as treatments. Not blocking — can ship as PCleanup.5.3-style spec hygiene.
- **No code changes to revert.** All commits ae55393 → df016db stand. The PCleanup.1.1 doc comment gets an update in the PCleanup.1.3 follow-up commit to reflect the reserved status.

---

## Migration path

1. **PCleanup.1.3.1** (now): Add `Effect::Treatment { id, params }` variant + serde-compat + dispatch arm wiring into the existing `TreatmentPipeline`. Scope: the trivial-per-layer treatments (6 of 9).
2. **PCleanup.1.3.2** (follow-up): Wire treatment-picker UI in the Effect chain. Operator-visible surface.
3. **PCleanup.W2-revisited** (follow-up): Land `fluid_warp` (compute-extension to TreatmentPipeline) and the simpler sibling treatments (ripple_lens, edge_lens, spotlights, drift_pinholes, edge_sparks, field_advect_source, collision_ripples, zone_brighten, zone_lens, portal_warp). Each is a separate small PR following the Phase 2 four-file pattern (one shader, one descriptor entry, one dispatch arm if needed, one set of tests).
4. **Spec hygiene**: update `004-phase-cleanup.md` W2 task descriptions to reference this decision doc and note the Treatment placement.
5. **Optionally** (later, post-cleanup): if operator feedback shows demand for true adjustment-layer semantics (warp the *scene below*, not just the layer's own photo), open a separate Phase 8 decision doc for Option A. Treatments-with-compute is forward-compat with that future path.

---

## Constraints honoured

- `src/render/CLAUDE.md` — `Effect::*` and `FxFamily::*` remain enums (no `dyn` dispatch). Adding `Effect::Treatment` to the enum keeps compile-time exhaustiveness for new variants.
- `src/project/CLAUDE.md` — `Effect::Treatment` needs a `ReverseStorage` Reverse rule per the v3 Mutation contract. PCleanup.1.3 must add the Mutation arm and proptest coverage following the `Effect::Color` whole-enum pattern (rule 1 of the three Reverse rules).
- The `intermediate_view` per-layer ping-pong stays compatible — multi-pass treatments (`blur_mask`, future compute treatments) reuse it as they already do.
- No tokio. Treatments are synchronous render-graph passes; the compute extension (for `fluid_warp`'s velocity field) uses the existing wgpu compute-pipeline pattern under `pollster::block_on` where needed.
