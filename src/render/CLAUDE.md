# `src/render/` — GPU lifecycle & render graph

## GPU bring-up split — `GpuContext` first, then surface, then `Renderer`

The plan-document signature `Renderer::new(surface)` is **impossible** against `OutputWindow::new`, which takes `&Instance / &Adapter / &Device` — the device must exist *before* the surface. Do not "fix" this by inverting the order; the split is the resolution.

1. **`GpuContext::new()`** — Instance + Adapter + Device + Queue. Bootstraps wgpu without a surface compatibility hint (`compatible_surface: None`); acceptable on macOS / desktop where adapters are surface-agnostic. Uses `pollster::block_on` internally — **the spec forbids tokio**.
2. **`OutputWindow::new(active_loop, monitor, &gpu.instance, &gpu.adapter, &gpu.device, windowed)`** — creates the `Surface`, picks a format, configures it.
3. **`Renderer::new(gpu, surface_format)`** — takes ownership of `GpuContext` and builds per-pass pipelines.

## Surface-acquisition outcomes (wgpu 29 `CurrentSurfaceTexture`)

- `Lost` / `Outdated` / `Suboptimal` → `RenderError::Surface{Lost, Outdated, Suboptimal}`. **Recoverable** via `OutputWindow::recreate_surface`. The App layer pattern-matches these and reconfigures.
- `Validation` → `RenderError::Surface(...)`. **Unrecoverable** at this layer (a device error scope was already raised).
- `Timeout` → log a warning, return `Ok(())`. Frame drop is fine.
- `Occluded` → trace, return `Ok(())`. Window minimized; nothing to draw.

`Suboptimal` returns `RenderError::SurfaceSuboptimal` *without* drawing — wgpu docs discourage drawing on a suboptimal surface; the next frame should come back as `Success` after reconfigure.

## Per-frame render-graph order

Per `EditingState`, every frame:

1. Each `LayerState` rasters/uploads its source (SVG via off-thread worker; image direct).
2. Layer's `effects: Vec<EffectNode>` runs as a ping-pong chain (`pipeline.rs`, `RenderCtx { source_view, dst_view, intermediate_view, sdf_view, zone_role, seed, t_layer_added_secs, overlay_view, collage_views }`). The third `intermediate_view` is for multi-pass effects (currently only `Blur`). The six new fields supply per-layer context to `Effect::Treatment` nodes:
   - `sdf_view` — signed-distance-field texture view computed once by `WarpRenderer::sync_from_layer` before the loop; reused by every node.
   - `zone_role` — `Option<ZoneRole>` from `cfg.warp.zone_role`; zone-keyed presets (`zone_brighten`, `zone_lens`) use this.
   - `seed` — `layer_id.0` as `u64`; drives per-layer randomisation in particle/noise treatments.
   - `t_layer_added_secs` — seconds since layer was added; `0.0` for Image/Video (not meaningful), non-zero for generative FX layers.
   - `overlay_view` — texture view of `Effect::Treatment.overlay_path`, uploaded via `image_texture_cache.lookup_or_upload`; `None` when path absent.
   - `collage_views` — slice of texture views for `Effect::Treatment.collage_paths`; length 0-4.

   **`identity_fit_uniform` invariant:** `svg_pipeline` owns layer fit (Cover/Contain/Stretch + focal point) and writes the fitted source into the ping-pong buffer. Chain effects (including `Effect::Treatment`) always receive an identity fit uniform — they operate on the already-fitted pixels and must not re-apply Cover/Contain, which would double-apply the fit transform.
3. `Compositor` blends layers in order with their `BlendMode` and `opacity`.
4. One or more `WarpRenderer`s render into the shared `warp_rt`. **First uses `LoadOp::Clear`; subsequent use `LoadOp::Load`** so multiple non-overlapping `source_rect` regions co-exist on one projector. (Roadmap defers true multi-output until single-surface UX is mature.)
5. `GammaPipeline` master pass.
6. Optional `OverlayPipeline` editor chrome (`O` toggles).

Effects are an **enum** (`Effect::{Color, Tint, Blur, Transform, External}`), not trait objects. Adding a variant without updating the renderer fails at compile time — preserve this property; do not move to dyn dispatch.

## Show-day frame wrapper — do not bypass

`crate::show_day::panic_restore::run_frame_assert_unwind_safe` wraps every render frame so a panic in a layer effect or shader handler becomes `RenderError::RenderPanic` instead of unwinding the event loop. A event-day panic must drop one frame, not crash the projector. Refactors that bypass the wrapper must explicitly re-establish the invariant. The unit test `render_panic_does_not_propagate` exists specifically to catch refactors that lose this.

## `warp_rt_view` as the egui thumbnail surface (V31.8.1)

After pass 4 above, `warp_rt_view` is pixel-equivalent to projector output. `register_scene_preview` (in `src/app.rs`) registers this view with the control window's egui renderer (`FilterMode::Linear`) so egui can draw it at any size. This is **the single source of truth** for both the Scene tab and the V31.8.2 top-chrome thumbnail — no extra blit or downsampled texture is needed; egui's sampler handles downsampling at draw time. The registration is re-run after every `resize_m5_gpu` (the texture is recreated on resize). See `EditingState::scene_texture_id` for the full consumer contract.

## Build-time WGSL validation

`build.rs` runs naga `parse_str` + `Validator` over every `.wgsl` in `src/render/shaders/` during `cargo build`. A broken shader fails the build instead of crashing the renderer at startup. Editing a `.wgsl` triggers a rebuild via `cargo:rerun-if-changed`. Do not skip validation by switching to `include_str!` without naga — runtime shader-compile errors in wgpu 29 surface asynchronously via `device.on_uncaptured_error`, not as a synchronous `Result`, and would land at the worst possible moment.
