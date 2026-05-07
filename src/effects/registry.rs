//! Extension-pass registry for [`Effect::External`].
//!
//! v1 ships no built-in `External` passes — the four production effects
//! (`Color` / `Tint` / `Blur` / `Transform`) are closed-enum dispatched
//! from `effects::Effect::render`. The roadmap explicitly defers a deep
//! generic shader graph editor, so the registry intentionally exposes a
//! narrow trait: a future plugin or in-tree extension implements
//! [`ExternalPass`] and registers it under a stable string `id`. The
//! schema's `Effect::External { id, params }` variant carries the
//! lookup key plus per-instance params (any JSON shape — the pass owns
//! its own deserialization, like the modulator system does for
//! parameter binding).
//!
//! Projects authored without External effects pay nothing: the registry
//! is built once at startup as empty by default, and the dispatch in
//! `Effect::render` is a `HashMap::get` followed by a `tracing::warn!`
//! and no-op when the id isn't found.

use std::collections::HashMap;

use crate::clock::Clock;

/// One named extension render pass.
///
/// The signature deliberately takes individual wgpu handles rather than
/// the parent `RenderCtx<'_>` to keep the registry-lookup borrow disjoint
/// from the field reborrows the dispatcher passes through here. The
/// dispatcher inside `Effect::render` does
/// `ctx.external_registry.get(id)` and calls this trait method while
/// reborrowing other ctx fields; that's only legal when this method
/// doesn't take `&mut RenderCtx`.
///
/// Implementations should:
/// - Return `false` from [`ExternalPass::writes_destination`] when the
///   pass intentionally leaves `dst_view` untouched, so the effect-
///   pipeline ping-pong doesn't flip on a no-write pass.
/// - Treat `params` as instance configuration; v1 makes no caching
///   contract.
pub trait ExternalPass: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        params: &serde_json::Value,
        clock: &Clock,
    );

    fn writes_destination(&self, _params: &serde_json::Value) -> bool {
        true
    }
}

/// Lookup table keyed on the stable id stored in
/// [`Effect::External::id`]. Empty by default — v1 has no built-in
/// passes; M7 / future plugin work registers entries at app startup.
#[derive(Default)]
pub struct ExternalRegistry {
    entries: HashMap<String, Box<dyn ExternalPass>>,
}

impl ExternalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pass under `id`. Replaces any previous registration.
    /// Used at app startup; not stable as a per-frame call (the v1
    /// dispatcher does not synchronize concurrent registry mutation).
    /// Held for the future plugin loader; v1 has no built-in callers.
    #[allow(dead_code)]
    pub fn register(&mut self, id: impl Into<String>, pass: Box<dyn ExternalPass>) {
        self.entries.insert(id.into(), pass);
    }

    pub fn get(&self, id: &str) -> Option<&dyn ExternalPass> {
        self.entries.get(id).map(|b| &**b)
    }

    /// Number of registered passes. Reserved for diagnostics + future
    /// plugin-status UI; not on a hot path.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
