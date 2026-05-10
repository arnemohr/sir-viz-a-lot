//! Native macOS menu bar skeleton — `NSMenu` / `NSMenuItem` wiring.
//!
//! ## Responsibility split across V31.4.x tasks
//!
//! | Task     | Responsibility                                              |
//! |----------|-------------------------------------------------------------|
//! | V31.4.1  | Create empty App / File / Edit / Window / Help submenus   |
//! |          | and install them as the application's main menu via        |
//! |          | `NSApplication::setMainMenu`.                              |
//! | V31.4.2  | **This file.** Wire File menu actions (Save, Save as…,     |
//! |          | Open, Quit) with keyboard equivalents. Actions push into   |
//! |          | a static `MENU_QUEUE`; `drain_pending()` is called from    |
//! |          | `App::about_to_wait` each frame.                           |
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
//!
//! ## Action queue (V31.4.2)
//!
//! AppKit menu callbacks fire on the main thread but outside the winit event
//! loop. To route them into rmap's state machine without locking `App`, the
//! callbacks push a `MenuAction` into `MENU_QUEUE` (a process-wide
//! `Mutex<Vec<MenuAction>>`). `drain_pending()` swaps the queue out each
//! `about_to_wait` tick and returns the drained actions for dispatch.
//!
//! `MenuTarget` is a tiny NSObject subclass (no Rust ivars) that owns one
//! selector per File menu action. A single leaked `Retained<MenuTarget>`
//! lives for the process lifetime; `setTarget` is called once per menu item
//! during `install_main_menu`.
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

use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{ClassType, MainThreadMarker, define_class, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

// ── Action queue ──────────────────────────────────────────────────────────────

/// An action emitted by a File menu item. Pushed by the NSObject selector
/// callback; drained each `about_to_wait` tick.
///
/// V31.4.3 will add `Undo`, `Redo` here; V31.4.4 adds `OpenHelp`, `ShowAbout`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Save,
    SaveAs,
    Open,
    Quit,
}

/// Process-wide pending-action queue.
///
/// Initialised on first use via `OnceLock`. The `Mutex` is held only for the
/// duration of a `push` or a `drain_pending` swap — never across a frame
/// boundary — so contention is negligible.
static MENU_QUEUE: OnceLock<Mutex<Vec<MenuAction>>> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<MenuAction>> {
    MENU_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn push(action: MenuAction) {
    if let Ok(mut q) = queue().lock() {
        q.push(action);
    }
}

/// Drain all pending menu actions and return them.
///
/// Called from `App::about_to_wait` on every tick. Returns an empty `Vec`
/// when no menu items have been activated since the last drain.
pub fn drain_pending() -> Vec<MenuAction> {
    match queue().lock() {
        Ok(mut q) => std::mem::take(&mut *q),
        Err(_) => Vec::new(),
    }
}

// ── NSObject subclass ─────────────────────────────────────────────────────────

// `RmapMenuTarget` — a minimal NSObject subclass whose selectors push into
// `MENU_QUEUE`. No Rust ivars are needed. A single instance is created in
// `install_main_menu`, leaked for process lifetime, and set as the `target`
// of every File menu item. The selectors are named with trailing colons
// because AppKit passes the sender (the `NSMenuItem`) as the first argument;
// the sender is ignored.
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RmapMenuTarget"]
    pub struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(saveAction:))]
        fn save_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::Save);
        }

        #[unsafe(method(saveAsAction:))]
        fn save_as_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::SaveAs);
        }

        #[unsafe(method(openAction:))]
        fn open_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::Open);
        }

        #[unsafe(method(quitAction:))]
        fn quit_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::Quit);
        }
    }
);

/// Leak a `Retained<MenuTarget>` and return a `'static` raw pointer.
///
/// `NSMenuItem::setTarget` does **not** retain its target (weak reference, per
/// AppKit contract). We need the target to live for the process lifetime;
/// leaking via `Retained::into_raw` achieves that without an unsafe static
/// `Retained<T>` (which fights `Send`/`Sync` even though `MenuTarget` is
/// thread-safe). We intentionally allow the leak — the singleton is reclaimed
/// by the OS on process exit.
fn leak_menu_target() -> *const MenuTarget {
    // SAFETY: `alloc()` + `msg_send![…, init]` is the standard Objective-C
    // two-step init. `MenuTarget` has no ivars; `NSObject -init` returns self.
    let target: Retained<MenuTarget> = unsafe {
        let allocated: *mut MenuTarget = objc2::msg_send![MenuTarget::class(), alloc];
        let inited: *mut MenuTarget = objc2::msg_send![allocated, init];
        Retained::from_raw(inited).expect("MenuTarget -init returned nil")
    };
    Retained::into_raw(target) as *const MenuTarget
}

/// Global singleton target, initialised once.
static MENU_TARGET_PTR: OnceLock<usize> = OnceLock::new();

// SAFETY: `*const MenuTarget` is only ever read (via coercion to `&AnyObject`)
// from the main thread. We store it as `usize` to satisfy `Send + Sync`.
fn menu_target_ref() -> &'static AnyObject {
    let addr = MENU_TARGET_PTR.get_or_init(|| leak_menu_target() as usize);
    // SAFETY: the pointer was created by `leak_menu_target` above, is non-null,
    // and lives for the process lifetime.
    unsafe { &*(*addr as *const AnyObject) }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Build and install the application's main menu bar.
///
/// V31.4.2: populates the File submenu with Save, Save as…, Open, and Quit.
/// Actions are routed via a static `Mutex<Vec<MenuAction>>` queue that
/// `App::about_to_wait` drains each tick.
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
    // Ensure queue is initialised before any callback can fire.
    let _ = queue();

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
    // V31.4.2: wire Save, Save as…, Open, Quit.
    {
        let file_item = NSMenuItem::new(mtm);
        let file_title = NSString::from_str("File");
        file_item.setTitle(&file_title);
        let file_submenu = NSMenu::new(mtm);
        file_submenu.setTitle(&file_title);

        let target: &AnyObject = menu_target_ref();

        // Save — Cmd-S
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Save"));
            item.setKeyEquivalent(&NSString::from_str("s"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(saveAction:)));
            }
            file_submenu.addItem(&item);
        }

        // Save as… — Cmd-Shift-S
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Save as\u{2026}"));
            item.setKeyEquivalent(&NSString::from_str("S")); // capital S
            item.setKeyEquivalentModifierMask(NSEventModifierFlags(
                NSEventModifierFlags::Command.0 | NSEventModifierFlags::Shift.0,
            ));
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(saveAsAction:)));
            }
            file_submenu.addItem(&item);
        }

        // Open — Cmd-O
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Open\u{2026}"));
            item.setKeyEquivalent(&NSString::from_str("o"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(openAction:)));
            }
            file_submenu.addItem(&item);
        }

        // Quit — Cmd-Q
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Quit"));
            item.setKeyEquivalent(&NSString::from_str("q"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(quitAction:)));
            }
            file_submenu.addItem(&item);
        }

        file_item.setSubmenu(Some(&file_submenu));
        menu_bar.addItem(&file_item);
    }

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
        "004-V31.4.2: main menu installed (File: Save/Save as…/Open/Quit + App/Edit/Window/Help)"
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

    /// V31.4.2 acceptance: the File submenu must contain exactly the four
    /// expected items with the right titles and key equivalents.
    ///
    /// Skips gracefully when not on the main thread (same pattern as above).
    #[test]
    #[cfg(target_os = "macos")]
    fn file_menu_has_four_items() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app
            .mainMenu()
            .expect("mainMenu must be Some after install_main_menu");

        // Find the File submenu (second top-level item, after the App item).
        let count = main_menu.numberOfItems();
        let mut file_submenu = None;
        for i in 0..count {
            if let Some(item) = main_menu.itemAtIndex(i) {
                if let Some(submenu) = item.submenu() {
                    if submenu.title().to_string() == "File" {
                        file_submenu = Some(submenu);
                        break;
                    }
                }
            }
        }

        let file_submenu = file_submenu.expect("File submenu must exist after install_main_menu");

        // Collect (title, key_equivalent) pairs.
        let item_count = file_submenu.numberOfItems();
        assert_eq!(
            item_count, 4,
            "File submenu must have exactly 4 items, found {}",
            item_count
        );

        let expected: &[(&str, &str)] = &[
            ("Save", "s"),
            ("Save as\u{2026}", "S"),
            ("Open\u{2026}", "o"),
            ("Quit", "q"),
        ];

        for (i, &(exp_title, exp_key)) in expected.iter().enumerate() {
            let item = file_submenu
                .itemAtIndex(i as isize)
                .expect("item must exist at index");
            let title = item.title().to_string();
            let key = item.keyEquivalent().to_string();
            assert_eq!(
                title, exp_title,
                "item {i}: expected title {:?}, got {:?}",
                exp_title, title
            );
            assert_eq!(
                key, exp_key,
                "item {i}: expected key equivalent {:?}, got {:?}",
                exp_key, key
            );
        }
    }

    /// V31.4.2 acceptance: the `MENU_QUEUE` round-trip — push three actions,
    /// drain, verify order is preserved and the queue is empty after drain.
    ///
    /// Starts by draining any stale state left by parallel tests.
    #[test]
    fn menu_action_queue_drain_round_trip() {
        // Clear any leftovers from parallel tests.
        let _ = drain_pending();

        push(MenuAction::Save);
        push(MenuAction::Open);
        push(MenuAction::Quit);

        let drained = drain_pending();
        assert_eq!(
            drained,
            vec![MenuAction::Save, MenuAction::Open, MenuAction::Quit],
            "drained actions must be in insertion order"
        );

        // Queue must be empty after drain.
        assert!(
            drain_pending().is_empty(),
            "queue must be empty after drain"
        );
    }
}
