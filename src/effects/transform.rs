//! 2D affine transform applied as a vertex-stage matrix multiplication on
//! the layer's quad. Translation is static; rotation and scale are
//! Modulator-driven.
//!
//! TODO(M4): build a `glam::Mat3` from current Modulator values; push as
//! uniform.
