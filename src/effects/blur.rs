//! Separable gaussian blur: horizontal pass then vertical pass into ping-pong
//! textures. Kernel size derived from `radius_px` Modulator at frame time.
//!
//! TODO(M4): WGSL kernel + bind group + pipeline for the separable gaussian.
