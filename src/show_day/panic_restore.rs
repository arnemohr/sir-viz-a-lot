//! `catch_unwind` wrapper around the per-frame render call. Per spec
//! §6 (Show-day requirements) and §"Error handling": a panic in the
//! renderer must NOT take the show down — convert it into a structured
//! [`RenderError::RenderPanic`] and let `App` log + recover (M2 path
//! through the existing render-error arm in `App::window_event`).
//!
//! T-M2-03 will wire this around `Renderer::render_frame` inside
//! `Renderer::render_to`. T-M2-10 will surface the resulting error on
//! the (still-future) egui control-window error overlay.

use std::panic::{AssertUnwindSafe, UnwindSafe, catch_unwind};

use crate::render::RenderError;

/// Run `f` under `catch_unwind`. If `f` panics, convert the panic
/// payload into `RenderError::RenderPanic { message }` instead of
/// unwinding past this call. If `f` returns `Err(e)`, that error
/// propagates unchanged. If `f` returns `Ok(())`, this returns
/// `Ok(())`.
///
/// `F: UnwindSafe` is the strict bound the standard library enforces.
/// Real call sites (T-M2-03) will wrap their closure in
/// [`AssertUnwindSafe`] when the renderer itself is not lexically
/// unwind-safe (interior `&mut` state etc.); this is the canonical
/// wgpu/winit pattern and accepts the small risk that a panic mid-
/// frame leaves the renderer in a logically inconsistent state. M2's
/// next frame will reconfigure / reinitialize as needed.
pub fn run_frame<F>(f: F) -> Result<(), RenderError>
where
    F: FnOnce() -> Result<(), RenderError> + UnwindSafe,
{
    match catch_unwind(f) {
        Ok(result) => result,
        Err(payload) => {
            let message = payload_to_message(&payload);
            tracing::error!(%message, "renderer panicked; converted to RenderPanic");
            Err(RenderError::RenderPanic { message })
        }
    }
}

/// Best-effort extraction of a human-readable message from a
/// `Box<dyn Any + Send>` panic payload. Handles the two common shapes
/// (`&'static str` and `String`); falls back to a generic note.
fn payload_to_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic with non-string payload".to_string()
    }
}

/// Convenience wrapper: most call sites have an `&mut Renderer` whose
/// closure body is not lexically `UnwindSafe`. This trampoline applies
/// `AssertUnwindSafe` so the call site reads cleanly.
pub fn run_frame_assert_unwind_safe<F>(f: F) -> Result<(), RenderError>
where
    F: FnOnce() -> Result<(), RenderError>,
{
    run_frame(AssertUnwindSafe(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_passes_through() {
        let result = run_frame_assert_unwind_safe(|| Ok(()));
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn err_passes_through() {
        let result =
            run_frame_assert_unwind_safe(|| Err(RenderError::Surface("test surface error".into())));
        assert!(matches!(result, Err(RenderError::Surface(_))));
    }

    #[test]
    fn panic_becomes_error_not_unwind() {
        let result = run_frame_assert_unwind_safe(|| {
            panic!("synthetic panic for the test");
        });
        match result {
            Err(RenderError::RenderPanic { message }) => {
                assert!(
                    message.contains("synthetic panic for the test"),
                    "expected message to contain the panic literal, got: {message}"
                );
            }
            other => panic!("expected RenderPanic, got {other:?}"),
        }
    }

    #[test]
    fn panic_with_string_payload_extracted() {
        let result = run_frame_assert_unwind_safe(|| {
            panic!("{}", String::from("string-payload panic"));
        });
        match result {
            Err(RenderError::RenderPanic { message }) => {
                assert!(message.contains("string-payload panic"));
            }
            other => panic!("expected RenderPanic, got {other:?}"),
        }
    }
}
