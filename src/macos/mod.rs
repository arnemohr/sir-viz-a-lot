//! macOS-specific platform integration for `rmap`.
//!
//! This directory holds all code that calls into Apple frameworks via the
//! `objc2` family (`objc2`, `objc2-foundation`, `objc2-app-kit`). Nothing in
//! here is compiled on Linux / Windows — the parent module declaration in both
//! `src/main.rs` and `src/lib.rs` is `#[cfg(target_os = "macos")]`-gated.
//!
//! ## Module layout
//!
//! - `menu` — native `NSMenu` / `NSMenuItem` skeleton installed at app boot
//!   (V31.4.1). Actions are wired in V31.4.2 – V31.4.4. V31.4.5 audits
//!   cfg-gating across the entire directory.
//!
//! ## Philosophy
//!
//! Each module mirrors `src/monitors/macos.rs` in style: small, self-contained,
//! single entry-point free function. No abstraction layers are exposed to the
//! rest of the codebase — callers import `crate::macos::menu::install_main_menu`
//! directly.

pub mod menu;
