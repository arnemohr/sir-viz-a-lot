//! Native macOS menu bar skeleton — `NSMenu` / `NSMenuItem` wiring.
//!
//! ## Responsibility split across V31.4.x tasks
//!
//! | Task     | Responsibility                                              |
//! |----------|-------------------------------------------------------------|
//! | V31.4.1  | **This file.** Create empty App / File / Edit / Window /   |
//! |          | Help submenus and install them as the application's main   |
//! |          | menu via `NSApplication::setMainMenu`.                     |
//! | V31.4.2  | Wire File menu actions (New, Open, Save, Save As, Close,   |
//! |          | Quit) with their keyboard equivalents.                     |
//! | V31.4.3  | Wire Edit menu actions (Undo, Redo, Cut, Copy, Paste, …).  |
//! | V31.4.4  | Wire Window / Help menu actions; call `setWindowsMenu` to  |
//! |          | enable macOS-managed Minimise / Zoom items.                |
//! | V31.4.5  | Audit cfg-gating across the entire `src/macos/` directory. |
//!
//! ## Layout
//!
//! The installed menu bar has five top-level items:
//!
//! ```text
//! [App]  File  Edit  Window  Help
//! ```
//!
//! The `[App]` (bold) submenu is included so that the system-provided
//! "About", "Hide", "Quit" items remain available once V31.4.2 populates it.
//! For V31.4.1 all five submenus are **empty** — no actions yet.
//!
//! ## winit interaction
//!
//! winit 0.30 installs its own minimal NSMenu on macOS (providing Cmd-Q and
//! "Quit rmap"). Calling `setMainMenu` with our new skeleton replaces winit's
//! menu entirely. **Cmd-Q is therefore unavailable from V31.4.1 until
//! V31.4.2 adds Quit to the File submenu.** This is accepted by the spec.
//!
//! ## Idempotency
//!
//! `setMainMenu` replaces unconditionally. Calling `install_main_menu` twice
//! simply reinstalls with the same structure — no guard is needed.
//!
//! ## Platform note
//!
//! The parent module (`src/macos/mod.rs`) is declared with
//! `#[cfg(target_os = "macos")]` in both `src/main.rs` and `src/lib.rs`, so
//! this file is never compiled on Linux or Windows. V31.4.5 will audit this
//! gating end-to-end.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

/// Build and install the application's main menu bar.
///
/// Creates five empty submenus — App, File, Edit, Window, Help — and
/// registers them as the main menu via
/// `NSApplication::sharedApplication().setMainMenu()`. Actions are wired in
/// V31.4.2 – V31.4.4.
///
/// # Thread safety
///
/// `MainThreadMarker` proves we are on the main thread. AppKit's NSMenu
/// and NSMenuItem are both `MainThreadOnly`. In practice this function is
/// called from `App::resumed`, which winit guarantees fires on the main
/// thread on macOS.
///
/// # Idempotency
///
/// Safe to call more than once: `setMainMenu` replaces any existing menu.
pub fn install_main_menu(mtm: MainThreadMarker) {
    // Build the top-level (invisible) menu bar container.
    let menu_bar = NSMenu::new(mtm);

    // ── App submenu ────────────────────────────────────────────────────────
    // The bold first entry carries the application name as its title. macOS
    // uses the *submenu's* title for display. The top-level item title is
    // conventionally left empty or set to the app name; we match what Apple's
    // MainMenu.xib templates do: empty top-level title, app-name submenu title.
    // V31.4.2 will populate this with About, Services, Hide, Quit, etc.
    let app_item = NSMenuItem::new(mtm);
    let app_submenu = NSMenu::new(mtm);
    app_submenu.setTitle(&NSString::from_str("rmap"));
    app_item.setSubmenu(Some(&app_submenu));
    menu_bar.addItem(&app_item);

    // ── File submenu ───────────────────────────────────────────────────────
    add_empty_submenu(&menu_bar, "File", mtm);

    // ── Edit submenu ───────────────────────────────────────────────────────
    add_empty_submenu(&menu_bar, "Edit", mtm);

    // ── Window submenu ─────────────────────────────────────────────────────
    // V31.4.4 will call `setWindowsMenu` to enable macOS-managed items
    // (Minimise, Zoom, Bring All to Front). For now an empty submenu suffices.
    add_empty_submenu(&menu_bar, "Window", mtm);

    // ── Help submenu ───────────────────────────────────────────────────────
    add_empty_submenu(&menu_bar, "Help", mtm);

    // Install as the application's main menu.
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&menu_bar));

    tracing::debug!(
        "004-V31.4.1: main menu skeleton installed (App / File / Edit / Window / Help)"
    );
}

/// Create an `NSMenuItem` whose submenu is an empty `NSMenu` named `title`,
/// and append it to `bar`.
///
/// The top-level menu item's title must match the submenu's title for macOS
/// to render the label in the menu bar correctly.
fn add_empty_submenu(bar: &NSMenu, title: &str, mtm: MainThreadMarker) {
    let item = NSMenuItem::new(mtm);
    let submenu = NSMenu::new(mtm);
    let ns_title = NSString::from_str(title);
    item.setTitle(&ns_title);
    submenu.setTitle(&ns_title);
    item.setSubmenu(Some(&submenu));
    bar.addItem(&item);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V31.4.1 acceptance: after `install_main_menu`, the application's main
    /// menu must contain submenus titled "File", "Edit", "Window", and "Help".
    ///
    /// The test skips gracefully if it runs off the main thread (nextest can
    /// dispatch tests to background threads; `MainThreadMarker::new()` returns
    /// `None` in that case). Tests running on the main thread will exercise the
    /// full path.
    #[test]
    #[cfg(target_os = "macos")]
    fn install_main_menu_has_expected_submenus() {
        // Gracefully skip if off-thread.
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app
            .mainMenu()
            .expect("mainMenu must be Some after install_main_menu");

        // Collect all top-level submenu titles.
        let count = main_menu.numberOfItems();
        let mut submenu_titles: Vec<String> = Vec::new();
        for i in 0..count {
            let item = main_menu.itemAtIndex(i);
            if let Some(item) = item {
                if let Some(submenu) = item.submenu() {
                    submenu_titles.push(submenu.title().to_string());
                }
            }
        }

        for expected in &["File", "Edit", "Window", "Help"] {
            assert!(
                submenu_titles.iter().any(|t| t == expected),
                "main menu is missing a submenu titled {:?}; found: {:?}",
                expected,
                submenu_titles,
            );
        }
    }
}
