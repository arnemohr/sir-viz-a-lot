# Decision: zone-tag uniform layout for zone-aware FX shaders

**Status:** Recommendation made — awaiting sign-off before P3.3.1 starts.
**Affects:** P3.3.1 (zone-tag accessor in `sdf_helper.wgsl`), P3.3.2
(bind-group contract), P3.5.1–P3.5.3 (zone-consuming presets).

---

## Background

`004-phase-3.md` specifies that "effect shaders read zone tags from a
per-fragment uniform indexed by layer. Tag dispatch happens shader-side,
not on the CPU." It does not specify the data layout — i.e., which binding
slot, what type, or whether the tag lives in the existing `FxParamsUniform`
or a new binding.

The choice is load-bearing: it sets the bind-group contract that every
zone-consuming preset (W5) and the golden-image GPU test (W6) must agree
on. Changing it later would be a breaking churn on every preset shader.

---

## Options

### Option A — Extend `FxParamsUniform` with a `u32` zone tag

Pack the zone tag into the existing `FxParamsUniform` (currently 8 × f32,
32 bytes, slot 2). The u32 maps to `ZoneRole` (0 = None, 1–7 = the seven
roles).

**Pros:**
- Zero new binding slots.
- No changes to the bind-group layout struct in `fx_presets.rs`.

**Cons:**
- `FxParamsUniform` is currently 32 bytes (8 × f32). Adding a `u32` either
  displaces one of the eight float params or requires re-padding to 48 bytes.
  Every existing preset shader's `struct FxParams` block must change, plus
  the corresponding Rust `[f32; 8]` uniform must become a mixed type. This
  is a larger blast radius than it appears.
- Mixes concerns: the "params" uniform is slot 2's documented purpose; a
  zone tag is different in kind (categorical, not continuous). The
  module-level doc in `fx_presets.rs` already says "Each preset documents
  which fields it reads; unmapped fields stay zero" — a zone tag breaks
  the contract that all fields are f32.
- All non-zone-aware presets would silently receive a zone tag they don't
  use, with no structural guarantee that they ignore it.

### Option B — New dedicated binding at `@group(0) @binding(6)` (recommended)

Add an optional binding slot 6: a small `ZoneTagUniform { zone_tag: u32,
_pad: [u32; 3] }` (16 bytes, 16-byte aligned for WebGPU min-binding-size
rules). Only zone-aware presets bind slot 6; non-zone presets do not
include a slot 6 entry in their bind-group-layout descriptor.

This mirrors exactly how slot 4 (source texture, fragment-family only) and
slot 5 (particle SSBO, compute-particle family only) are already
family-conditional in the Phase 2 bind-group contract (see `fx_presets.rs`
line 13–22 canonical slot table).

**Pros:**
- `FxParamsUniform` (slot 2) stays byte-stable — no changes to existing
  preset shaders.
- Structural opt-in: a zone-aware preset must explicitly include slot 6
  in its layout; an accidental omission fails at pipeline build time, not
  silently at runtime.
- Clean extension: the existing slot 4 / slot 5 precedent makes Option B
  idiomatic for this codebase.
- `FxFamily` enum gains a new variant or flag to signal zone-awareness;
  `fx_registry()` entries declare it; the dispatch layer binds slot 6 only
  when the preset is zone-aware.

**Cons:**
- One more bind-group layout variant to test.
- New `ZoneTagUniform` Rust struct + wgpu `BufferUsages::UNIFORM` buffer
  per-layer (or per-frame write of a shared buffer). Small overhead; one
  16-byte write per zone-aware FX layer per frame.

### Option C — SSBO indexed by mask ID

A storage buffer whose entries map mask_polygon index → zone_role, shared
across all layers in the scene.

**Cons:**
- Over-engineered: zone role in Phase 3 is per-layer (one mask per layer),
  not a multi-mask index. Phase 4 scene grammars may revisit this; defer
  the complexity.
- Requires a scene-level GPU buffer (not per-layer), complicating the
  per-frame render graph.

---

## Recommendation

**Option B.** New binding at `@group(0) @binding(6)`, type
`ZoneTagUniform { zone_tag: u32, _pad: [u32; 3] }`, optional per the
`FxFamily` tag on the preset's registry entry.

The `sdf_helper.wgsl` accessor (`fn zone_tag() -> u32`) reads from slot 6
and returns 0 (None) when the caller doesn't bind it — this can't be done
in WGSL (you can't conditionally read from an unbound slot), so the
accessor is instead a WGSL snippet injected only into zone-aware preset
shaders at pipeline build time, exactly as `SDF_HELPER_WGSL` is injected
today. Non-zone-aware shaders never see the snippet and their bind-group
layout omits slot 6.

The constant definitions (`ZONE_NONE: u32 = 0u`, `ZONE_WINDOW: u32 = 1u`,
etc.) live in a new `ZONE_TAG_WGSL` string constant in `src/render/sdf.rs`
alongside `SDF_HELPER_WGSL`, injected before shader source for zone-aware
presets.

**Dependent task:** P3.3.1 implements the `ZONE_TAG_WGSL` constant and
accessor; P3.3.2 updates the canonical bind-group contract table in
`fx_presets.rs` to document slot 6.
