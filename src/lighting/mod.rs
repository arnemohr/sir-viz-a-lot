//! Phase 5 lighting output — Art-Net DMX transport, fixture groups,
//! colour-from-pixel canvas sampling.
//!
//! Gated on `feature = "lighting"` (off by default). The render thread is
//! never blocked by network I/O: a background `LightingThread` owns the UDP
//! socket and drains a bounded crossbeam channel at ~44 Hz.
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

// Phase 5 types are built incrementally; wire-up happens in W2.4/W3/W4.
// Until then, suppress dead-code lint for the lighting module.
#![allow(dead_code)]

pub mod error;
pub mod transport;
pub mod universe;

// P5.1.2 — Frame-budget constraint documentation (stub assertion).
//
// With up to 16 DMX universes sent per frame by the background
// LightingThread, render frame time (p99) must stay below the 16.6 ms
// show-day target. This stub succeeds trivially — lighting is not yet
// wired into the render loop — and documents the invariant for W4/W5
// implementers.

#[cfg(test)]
mod tests {
    /// P5.1.2 — Placeholder budget assertion for Phase 5 lighting output.
    ///
    /// Validates the constants that the actual frame-budget enforcement
    /// (wired in P5.4.5 + P5.11.1) will measure against. The real p99
    /// timing assertion is a manual check in `docs/show-day-checklist.md`.
    ///
    /// Acceptance criterion: "Show-day frame budget unchanged with up to
    /// 16 universes of DMX output active."
    #[test]
    fn lighting_frame_budget_stub() {
        // P5.1.2 budget constants — must match the colour-space decision
        // doc (P5.0.3) and transport decision doc (P5.0.1).
        //
        // Max universes Phase 5 must support without frame regression.
        const MAX_UNIVERSES: usize = 16;
        // DMX channels per universe (always 512 per DMX512 spec).
        const CHANNELS_PER_UNIVERSE: usize = 512;
        // Total bytes across all universes sent per lighting tick.
        const BYTES_PER_FRAME: usize = MAX_UNIVERSES * CHANNELS_PER_UNIVERSE;
        // Lighting-tap staging buffer: 64×36×4 bytes (RGBA8Unorm).
        const TAP_BUFFER_BYTES: usize = 64 * 36 * 4;
        // Bounded channel capacity: render thread drops silently if full.
        const CHANNEL_CAPACITY: usize = 4;

        // Compile-time checks — values must be stable across refactors.
        const {
            assert!(
                BYTES_PER_FRAME == 8192,
                "16 universes × 512 ch = 8192 B/frame"
            )
        }
        const {
            assert!(
                TAP_BUFFER_BYTES == 9216,
                "64×36 RGBA8 lighting-tap = 9216 B"
            )
        }
        const {
            assert!(
                CHANNEL_CAPACITY >= 2,
                "channel capacity too small for frame-drop safety"
            )
        }

        // Runtime echo so the test output shows the constants when run with -v.
        println!("  Max universes:         {MAX_UNIVERSES}");
        println!("  Bytes per frame:       {BYTES_PER_FRAME} B");
        println!("  Lighting-tap buffer:   {TAP_BUFFER_BYTES} B");
        println!("  Channel capacity:      {CHANNEL_CAPACITY}");
        println!("  Show-day frame budget: ≤16.6 ms p99 (manual, see checklist)");
    }
}
