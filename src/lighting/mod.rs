//! Phase 5 lighting output — Art-Net DMX transport, fixture groups,
//! colour-from-pixel canvas sampling.
//!
//! Gated on `feature = "lighting"` (off by default). The render thread is
//! never blocked by network I/O: a background [`thread::LightingThread`]
//! owns the UDP socket and drains a bounded crossbeam channel at ~44 Hz.
//!
//! # Architecture
//!
//! ```text
//! Render thread
//!     ──crossbeam try_send (bounded 4)──► LightingThread
//!                                              │
//!                                         UdpSocket (Art-Net port 6454)
//!                                              │
//!                                         ──► Art-Net node on LAN
//! ```
//!
//! All files in this module compile only when `cfg(feature = "lighting")`.
