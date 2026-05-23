# Decision: render approach for Effect::LightTrail — SDF vs multi-pass dash

**Status:** Decided — T3.2 implements against this plan.
**Affects:** T3.2 (pipeline + bind group), T3.3 (WGSL shader), T3.4 (glow / head compositing),
T3.5 (GPU golden test).

---

## Context

T1.3 established `LightTrailGpuPolyline` — a storage buffer of `[px, py, arclen]` f32 triples,
verified on Metal via wgpu 29. The shader in T3.3 must traverse this buffer to draw the comet.
Two approaches exist: a single-pass SDF that queries per-fragment distance to the visible trail
segment, or a multi-pass dash that rasterizes stroked geometry and then blurs. This choice is
load-bearing because it sets the bind-group contract that T3.2 through T3.5 all depend on.

---

## Option A — SDF, single full-screen fragment pass (recommended)

One render pass covering the full layer render-target. The polyline storage buffer (from T1.3)
is bound read-only. For each fragment, scan the visible window of polyline segments
(see windowed-scan constraint below), find the nearest point, and compute:

- `dist` — perpendicular distance to the segment (drives glow smoothstep and core AA)
- `arc_t` — arc-length position along the trail normalized 0..1 within the visible window (drives
  color, opacity fade, and gradient spread)

Color, glow halo, and head core are computed in-shader from these two scalars and blended
back-to-front per the §6 composition order (base → soft glow → sharp core → head halo → head
core). The result is written to `dst_view`; `Effect::render` returns `true` to advance ping-pong.

No MSAA, no intermediate render targets beyond the standard ping-pong pair.

## Option B — Multi-pass dash

Rasterize the polyline as a stroked mesh (either CPU-side geometry expansion or
vertex-shader expansion), producing a stroke image in `intermediate_view`. Then run a
separable Gaussian for the glow (similar to `BlurPipeline`). Requires at least two passes and
reads/writes `intermediate_view` as an intermediate target.

---

## Decision: Option A — SDF

### Data structure already lands here

T1.3 stores the polyline as `[px, py, arclen]` f32 triples in a storage buffer. That is
exactly the data structure an SDF query needs: for each fragment, iterate the windowed
segment list, project onto each segment, keep the nearest. A multi-pass dash approach either
ignores the arc-length dimension (wasting T1.3's work) or must reconstruct it from the
stroked mesh.

### One pass fits the Look chain ping-pong order without extra intermediates

`RenderCtx` provides `source_view`, `dst_view`, and `intermediate_view`. `Blur` uses
`intermediate_view` for its horizontal pass. The SDF approach needs only `source_view` and
`dst_view` — no `intermediate_view` write. This keeps the pipeline simple and avoids the
`LoadOp::Load` / `LoadOp::Clear` bookkeeping that a multi-pass effect requires
(see `src/render/CLAUDE.md`, per-frame render-graph order §2).

### Antialiasing is natural via smoothstep on distance

`smoothstep(stroke_width, stroke_width + aa_width, dist)` gives per-fragment sub-pixel AA
with no MSAA budget. Multi-pass dash would need MSAA or per-segment AA injection, neither
of which is trivially available in the current effect pipeline.

### Composes with `panic_restore` trivially

A single fragment shader with no compute pass and no persistent intermediate state has
nothing to leave in an inconsistent condition across frame boundaries. If the shader faults,
`panic_restore::run_frame_assert_unwind_safe` catches it and drops one frame.
Multi-pass effects with `intermediate_view` state could expose a stale/half-written texture
on the next frame after a panic.

---

## Constraint on T3.3 — windowed scan (LOAD-BEARING)

The fragment shader **must** restrict the polyline scan to the visible segment window
`[head_dist - trail_length * total_length, head_dist]` rather than iterating all
`sample_resolution` points for every fragment.

**Performance math:**

- Naive O(n·p) at 4K (≈8 M fragments) × `sample_resolution = 512`:
  ≈ **4 B segment-checks per frame** — well above 60-fps budget on Metal.
- Windowed at `trail_length = 0.2`: visible window ≈ 0.2 × 512 ≈ **100 segments**;
  100 × 8 M = **800 M segment-checks per frame** — comfortable at 60 fps on Metal.

This is a **constraint on T3.3**, not advice. A PR that ships a naive O(n·p) loop over all
`sample_resolution` entries must be bounced regardless of whether it passes golden tests on
the reviewer's machine.

Implementation: pass `head_dist` and `trail_length` through `LightTrailParams` (already
present as `progress` and `trail_length`). In WGSL, compute `start_idx` and `end_idx` from
those fields and the `total_length` stored in the UBO; loop only over
`polyline[start_idx..end_idx]`.

---

## Bind group layout

Mirrors `BlurPipeline` (`src/effects/blur.rs:69–99`) with an additional storage buffer slot.
All entries `visibility: wgpu::ShaderStages::FRAGMENT`.

| Binding | Type | Purpose |
|---------|------|---------|
| `@group(0) @binding(0)` | `wgpu::BindingType::Texture { sample_type: Float { filterable: true }, … }` | `source_view` — base layer pixels |
| `@group(0) @binding(1)` | `wgpu::BindingType::Sampler(SamplerBindingType::Filtering)` | clamp-to-edge linear sampler |
| `@group(0) @binding(2)` | `wgpu::BindingType::Buffer { ty: BufferBindingType::Uniform, … }` | `LightTrailParams`, 192 bytes |
| `@group(0) @binding(3)` | `wgpu::BindingType::Buffer { ty: BufferBindingType::Storage { read_only: true }, … }` | polyline storage buffer, `array<f32>` |

`intermediate_view` is **not** needed for the SDF single-pass approach. T3.2 must not
allocate or bind it unless a future multi-pass glow extension specifically requires it.

---

## Sample resolution caveat

Tight SVG curves at low `sample_resolution` facet under both approaches because the
underlying polyline approximation is coarser than the curve. This is not a discriminator
between SDF and dash — both degrade identically as `sample_resolution` falls. Document the
`sample_resolution` knob (default 512) as the primary quality tuning lever; operators with
hairpin-curve SVGs should increase it. The faceting appears as flat micro-segments on the
trail core, not as noise or drift.

---

## What T3.3 / T3.4 / T3.5 inherit from this decision

- **T3.3 (WGSL shader):** single-pass SDF; bind group at slots 0–3 above; windowed scan
  mandatory (O(n) over visible window only); smoothstep AA on `dist`; in-shader composition
  order per §6 of the prompt (base → glow → core → head halo → head core).
- **T3.4 (glow / head):** glow computed in the same pass as the core via a wider
  `smoothstep` on `dist` — no separate blur pass, no `intermediate_view`.
- **T3.5 (GPU golden):** one pipeline, one bind group per frame; golden image must be
  stable across save/load per acceptance criteria §9 round-trip requirement.

---

## Follow-ups

- **Spatial-hash binning.** If perf profiling shows the windowed linear scan is still too
  slow for very long trails (`trail_length` near 1.0), partition the polyline into a 1-D
  spatial hash (bucket by arc-length range) and binary-search the relevant bucket. Not
  needed at default params; revisit only with profiler data.
- **Head tangent AA (v2).** `align = true` rotates the head to the path tangent. A future
  pass could apply an oriented soft-ellipse SDF for the head rather than a circle, giving
  a more directional glow. Defer to post-v1.
- **Metal storage-buffer size limit.** wgpu 29 + Metal supports read-only storage buffers
  in fragment stages (confirmed T1.3). If a future macOS release or wgpu upgrade reverts
  this, fall back to a 1-D `texture_1d<f32>` carrying the same `[px, py, arclen]` packing.
  The shader logic is unchanged; only the binding type and WGSL accessor change.
