//! Borderless fullscreen output window on the chosen monitor. Hides the
//! cursor, owns blackout/freeze state, and recreates its surface on panic
//! or on `SurfaceError::Lost`/`SurfaceError::Outdated`.

#[derive(Debug, Default)]
pub struct OutputState {
    pub blackout: bool,
    pub freeze: bool,
}

impl OutputState {
    pub fn toggle_blackout(&mut self) {
        self.blackout = !self.blackout;
    }

    pub fn toggle_freeze(&mut self) {
        self.freeze = !self.freeze;
    }
}

// TODO(M1): wgpu Surface + winit Window held here; recreate on
// RedrawRequested when the surface is Lost or Outdated.
