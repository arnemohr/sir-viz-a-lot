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
//!   (V31.4.1). Actions wired in V31.4.2 – V31.4.4 (File / Edit / App / Help
//!   submenus, About panel, AppKit-managed Window menu). V31.4.5 audited
//!   cfg-gating across the directory: no per-symbol guards needed inside
//!   children — the parent's `cfg` at declaration excludes the whole
//!   subtree from non-macOS builds.
//!
//! ## Philosophy
//!
//! Each module mirrors `src/monitors/macos.rs` in style: small, self-contained,
//! single entry-point free function. No abstraction layers are exposed to the
//! rest of the codebase — callers import `crate::macos::menu::install_main_menu`
//! directly.

pub mod menu;
