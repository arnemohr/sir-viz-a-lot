//! P5.2.1 — `LightingError` — unified error type for the lighting module.

use thiserror::Error;

/// Errors that can arise in the lighting module.
#[derive(Debug, Error)]
pub enum LightingError {
    /// An I/O error from the UDP socket (e.g. `send_to` on a closed network).
    #[error("lighting I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An Art-Net PDU encoding failure (malformed data or universe number).
    #[error("art-net encode error: {0}")]
    Encode(String),

    /// The universe number is out of the valid Art-Net range (0..=32_767).
    #[error("invalid Art-Net universe number {0} (must be 0..=32_767)")]
    InvalidUniverse(u16),
}
