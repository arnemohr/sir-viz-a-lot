//! Monitor enumeration. Wraps winit's monitor list; on macOS, falls back to
//! `objc2-app-kit` for display names winit returns as None.

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub size: (u32, u32),
    pub position: (i32, i32),
    pub scale_factor: f64,
}

pub fn list() -> Vec<MonitorInfo> {
    // TODO(M1): wire to winit's available_monitors() inside an
    // ApplicationHandler context. On macOS, fall back to objc2-app-kit for
    // missing display names.
    Vec::new()
}
