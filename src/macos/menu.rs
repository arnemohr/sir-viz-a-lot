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
//! | V31.4.3  | Wire Edit menu actions (Undo Cmd-Z, Redo Cmd-Shift-Z).     |
//! |          | Cut/Copy/Paste are explicitly out of scope per the spec.   |
//! | V31.4.4  | Window menu via `setWindowsMenu`; Help menu (`rmap Help`   |
//! |          | via `open_help_url`); App-submenu About + Quit; standard   |
//! |          | about panel.                                               |
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
use objc2_app_kit::{
    NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion,
    NSAboutPanelOptionVersion, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSDictionary, NSString};

// ── Action queue ──────────────────────────────────────────────────────────────

/// An action emitted by a menu item. Pushed by the NSObject selector
/// callback; drained each `about_to_wait` tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Save,
    SaveAs,
    Open,
    Quit,
    Undo,
    Redo,
    OpenHelp,
    ShowAbout,
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

        #[unsafe(method(undoAction:))]
        fn undo_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::Undo);
        }

        #[unsafe(method(redoAction:))]
        fn redo_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::Redo);
        }

        #[unsafe(method(helpAction:))]
        fn help_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::OpenHelp);
        }

        #[unsafe(method(aboutAction:))]
        fn about_action(&self, _sender: *mut AnyObject) {
            push(MenuAction::ShowAbout);
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
/// V31.4.3: adds Undo/Redo to the Edit submenu.
/// V31.4.4: populates the App submenu (About + Quit, per macOS HIG), the Help
/// submenu (rmap Help), and registers the Window submenu via `setWindowsMenu`.
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
    //
    // V31.4.4: populated per macOS HIG — About at the top, Quit at the bottom.
    // Quit lives here (not in File) per the macOS Human Interface Guidelines.
    {
        let app_item = NSMenuItem::new(mtm);
        let app_submenu = NSMenu::new(mtm);
        app_submenu.setTitle(&NSString::from_str("rmap"));

        let target: &AnyObject = menu_target_ref();

        // About rmap — no key equivalent (macOS convention)
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("About rmap"));
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(aboutAction:)));
            }
            app_submenu.addItem(&item);
        }

        app_submenu.addItem(&NSMenuItem::separatorItem(mtm));

        // Quit rmap — Cmd-Q (moved from File submenu per macOS HIG)
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Quit rmap"));
            item.setKeyEquivalent(&NSString::from_str("q"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(quitAction:)));
            }
            app_submenu.addItem(&item);
        }

        app_item.setSubmenu(Some(&app_submenu));
        menu_bar.addItem(&app_item);
    }

    // ── File submenu ───────────────────────────────────────────────────────
    // V31.4.2: wire Save, Save as…, Open.
    // Note: Quit moved to the App submenu in V31.4.4 per macOS HIG.
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

        file_item.setSubmenu(Some(&file_submenu));
        menu_bar.addItem(&file_item);
    }

    // ── Edit submenu ───────────────────────────────────────────────────────
    // V31.4.3: wire Undo (Cmd-Z) and Redo (Cmd-Shift-Z).
    // Cut / Copy / Paste are explicitly out of scope per the spec.
    {
        let edit_item = NSMenuItem::new(mtm);
        let edit_title = NSString::from_str("Edit");
        edit_item.setTitle(&edit_title);
        let edit_submenu = NSMenu::new(mtm);
        edit_submenu.setTitle(&edit_title);

        let target: &AnyObject = menu_target_ref();

        // Undo — Cmd-Z
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Undo"));
            item.setKeyEquivalent(&NSString::from_str("z"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(undoAction:)));
            }
            edit_submenu.addItem(&item);
        }

        // Redo — Cmd-Shift-Z
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("Redo"));
            item.setKeyEquivalent(&NSString::from_str("Z")); // capital Z
            item.setKeyEquivalentModifierMask(NSEventModifierFlags(
                NSEventModifierFlags::Command.0 | NSEventModifierFlags::Shift.0,
            ));
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(redoAction:)));
            }
            edit_submenu.addItem(&item);
        }

        edit_item.setSubmenu(Some(&edit_submenu));
        menu_bar.addItem(&edit_item);
    }

    // ── Window submenu ─────────────────────────────────────────────────────
    // V31.4.4: register via `setWindowsMenu` so AppKit auto-populates
    // Minimise, Zoom, and Bring All to Front.
    {
        let window_item = NSMenuItem::new(mtm);
        let window_title = NSString::from_str("Window");
        window_item.setTitle(&window_title);
        let window_submenu = NSMenu::new(mtm);
        window_submenu.setTitle(&window_title);
        window_item.setSubmenu(Some(&window_submenu));
        menu_bar.addItem(&window_item);
        // Registering the submenu with NSApplication lets AppKit manage the
        // standard window items (Minimise, Zoom, Bring All to Front) for free.
        NSApplication::sharedApplication(mtm).setWindowsMenu(Some(&window_submenu));
    }

    // ── Help submenu ───────────────────────────────────────────────────────
    // V31.4.4: wire "rmap Help" (Cmd-Shift-?) to the same handler as the
    // v3 `?` toolbar button (`open_help_url` via `MenuAction::OpenHelp`).
    {
        let help_item = NSMenuItem::new(mtm);
        let help_title = NSString::from_str("Help");
        help_item.setTitle(&help_title);
        let help_submenu = NSMenu::new(mtm);
        help_submenu.setTitle(&help_title);

        let target: &AnyObject = menu_target_ref();

        // rmap Help — Cmd-Shift-?
        {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str("rmap Help"));
            item.setKeyEquivalent(&NSString::from_str("?"));
            item.setKeyEquivalentModifierMask(NSEventModifierFlags(
                NSEventModifierFlags::Command.0 | NSEventModifierFlags::Shift.0,
            ));
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(helpAction:)));
            }
            help_submenu.addItem(&item);
        }

        help_item.setSubmenu(Some(&help_submenu));
        menu_bar.addItem(&help_item);
    }

    // Install as the application's main menu.
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&menu_bar));

    tracing::debug!(
        "004-V31.4.4: main menu installed (App: About/Quit; File: Save/Save as\u{2026}/Open; Edit: Undo/Redo; Window: setWindowsMenu; Help: rmap Help)"
    );
}

// ── About panel ───────────────────────────────────────────────────────────────

/// Display the standard macOS About panel, populated with rmap metadata.
///
/// Version and license strings are read at compile time from `Cargo.toml`
/// via `env!`. No `LICENSE` file is needed at runtime.
///
/// The panel is owned by AppKit after this call; no reference is kept.
pub fn show_about_panel(mtm: MainThreadMarker) {
    // Build the options dictionary.
    //
    // Keys are `NSAboutPanelOptionKey` (type alias for `NSString`). Values
    // are heterogeneous NSObjects, so the dictionary is typed as
    // `NSDictionary<NSString, AnyObject>`.
    //
    // NOTE: "Copyright" is not a public `NSAboutPanelOptionKey` constant in
    // the AppKit headers, but AppKit honors it as a stable undocumented key.
    // We construct it by hand rather than relying on a missing constant.
    let version_str = NSString::from_str(env!("CARGO_PKG_VERSION"));
    let copyright_str = NSString::from_str("Licensed under MIT OR Apache-2.0");

    // SAFETY: Each `Retained<NSString>` is a subclass of `AnyObject`; the
    // cast reinterprets the pointer with the wider type without any ABI
    // difference. We do not expose the cast outside this block.
    let version_obj: Retained<AnyObject> = unsafe { Retained::cast_unchecked(version_str.clone()) };
    let version_obj2: Retained<AnyObject> =
        unsafe { Retained::cast_unchecked(version_str.clone()) };
    let copyright_obj: Retained<AnyObject> = unsafe { Retained::cast_unchecked(copyright_str) };

    // SAFETY: The three `extern "C" pub static` option-key pointers are
    // valid for the process lifetime (they are lazily resolved linker
    // symbols backed by AppKit). Reading them requires `unsafe` on Rust's
    // side because the compiler cannot verify they are properly initialised
    // before first use; AppKit guarantees that for all framework statics.
    let name_key: &NSString = unsafe { NSAboutPanelOptionApplicationName };
    let app_version_key: &NSString = unsafe { NSAboutPanelOptionApplicationVersion };
    let version_key: &NSString = unsafe { NSAboutPanelOptionVersion };
    let copyright_key = NSString::from_str("Copyright");

    let keys: &[&NSString] = &[name_key, app_version_key, version_key, &copyright_key];

    // Application name: use "rmap" (matches the App submenu title and
    // CFBundleName; AppKit would otherwise fall back to the process name
    // which may be the binary path in a non-bundled build).
    let name_str = NSString::from_str("rmap");
    let name_obj: Retained<AnyObject> = unsafe { Retained::cast_unchecked(name_str) };

    let values: &[Retained<AnyObject>] = &[name_obj, version_obj, version_obj2, copyright_obj];

    let opts = NSDictionary::<NSString, AnyObject>::from_retained_objects(keys, values);

    // SAFETY: The dictionary has the correct key/value types:
    // - Keys are `NSString` (= `NSAboutPanelOptionKey`).
    // - Values are `NSString` objects cast to `AnyObject`, which satisfies
    //   the heterogeneous-value contract of `orderFrontStandardAboutPanelWithOptions:`.
    unsafe {
        NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanelWithOptions(&opts);
    }
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

    /// V31.4.2 / V31.4.4 acceptance: the File submenu must contain exactly
    /// three items (Save, Save as…, Open). Quit moved to the App submenu.
    ///
    /// Skips gracefully when not on the main thread (same pattern as above).
    #[test]
    #[cfg(target_os = "macos")]
    fn file_menu_has_three_items() {
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
            item_count, 3,
            "File submenu must have exactly 3 items, found {}",
            item_count
        );

        let expected: &[(&str, &str)] = &[
            ("Save", "s"),
            ("Save as\u{2026}", "S"),
            ("Open\u{2026}", "o"),
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

    /// V31.4.4 acceptance: the App submenu (titled "rmap") must contain at
    /// least 3 items — About rmap / separator / Quit rmap — with the correct
    /// titles and key equivalents.
    ///
    /// Skips gracefully when not on the main thread.
    #[test]
    #[cfg(target_os = "macos")]
    fn app_menu_has_about_and_quit() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app
            .mainMenu()
            .expect("mainMenu must be Some after install_main_menu");

        // Find the App submenu (submenu titled "rmap").
        let count = main_menu.numberOfItems();
        let mut app_submenu = None;
        for i in 0..count {
            if let Some(item) = main_menu.itemAtIndex(i) {
                if let Some(submenu) = item.submenu() {
                    if submenu.title().to_string() == "rmap" {
                        app_submenu = Some(submenu);
                        break;
                    }
                }
            }
        }

        let app_submenu = app_submenu.expect("App submenu titled 'rmap' must exist");

        let item_count = app_submenu.numberOfItems();
        assert!(
            item_count >= 3,
            "App submenu must have at least 3 items (About / separator / Quit), found {}",
            item_count
        );

        // First item: About rmap
        let about_item = app_submenu
            .itemAtIndex(0)
            .expect("item 0 must exist in App submenu");
        assert_eq!(
            about_item.title().to_string(),
            "About rmap",
            "item 0 must be 'About rmap'"
        );

        // Last item: Quit rmap with key equivalent "q"
        let quit_item = app_submenu
            .itemAtIndex(item_count - 1)
            .expect("last item must exist in App submenu");
        assert_eq!(
            quit_item.title().to_string(),
            "Quit rmap",
            "last item must be 'Quit rmap'"
        );
        assert_eq!(
            quit_item.keyEquivalent().to_string(),
            "q",
            "Quit rmap must have key equivalent 'q'"
        );
    }

    /// V31.4.4 acceptance: the Help submenu must contain exactly one item
    /// ("rmap Help") with key equivalent "?".
    ///
    /// Skips gracefully when not on the main thread.
    #[test]
    #[cfg(target_os = "macos")]
    fn help_menu_has_one_item() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app
            .mainMenu()
            .expect("mainMenu must be Some after install_main_menu");

        // Find the Help submenu.
        let count = main_menu.numberOfItems();
        let mut help_submenu = None;
        for i in 0..count {
            if let Some(item) = main_menu.itemAtIndex(i) {
                if let Some(submenu) = item.submenu() {
                    if submenu.title().to_string() == "Help" {
                        help_submenu = Some(submenu);
                        break;
                    }
                }
            }
        }

        let help_submenu = help_submenu.expect("Help submenu must exist after install_main_menu");

        let item_count = help_submenu.numberOfItems();
        assert_eq!(
            item_count, 1,
            "Help submenu must have exactly 1 item, found {}",
            item_count
        );

        let item = help_submenu
            .itemAtIndex(0)
            .expect("item 0 must exist in Help submenu");
        assert_eq!(
            item.title().to_string(),
            "rmap Help",
            "Help submenu item 0 must be 'rmap Help'"
        );
        assert_eq!(
            item.keyEquivalent().to_string(),
            "?",
            "rmap Help must have key equivalent '?'"
        );
    }

    /// V31.4.4 acceptance: `setWindowsMenu` must have been called, so
    /// `NSApplication::windowsMenu()` returns Some after `install_main_menu`.
    ///
    /// Skips gracefully when not on the main thread.
    #[test]
    #[cfg(target_os = "macos")]
    fn window_menu_assigned() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        assert!(
            app.windowsMenu().is_some(),
            "NSApplication::windowsMenu() must return Some after install_main_menu calls setWindowsMenu"
        );
    }

    /// V31.4.3 acceptance: the Edit submenu must contain exactly two items
    /// (Undo, Redo) with the right key equivalents.
    ///
    /// Skips gracefully when not on the main thread (same pattern as above).
    #[test]
    #[cfg(target_os = "macos")]
    fn edit_menu_has_two_items() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        install_main_menu(mtm);

        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app
            .mainMenu()
            .expect("mainMenu must be Some after install_main_menu");

        // Find the Edit submenu.
        let count = main_menu.numberOfItems();
        let mut edit_submenu = None;
        for i in 0..count {
            if let Some(item) = main_menu.itemAtIndex(i) {
                if let Some(submenu) = item.submenu() {
                    if submenu.title().to_string() == "Edit" {
                        edit_submenu = Some(submenu);
                        break;
                    }
                }
            }
        }

        let edit_submenu = edit_submenu.expect("Edit submenu must exist after install_main_menu");

        // Collect (title, key_equivalent) pairs.
        let item_count = edit_submenu.numberOfItems();
        assert_eq!(
            item_count, 2,
            "Edit submenu must have exactly 2 items, found {}",
            item_count
        );

        let expected: &[(&str, &str)] = &[("Undo", "z"), ("Redo", "Z")];

        for (i, &(exp_title, exp_key)) in expected.iter().enumerate() {
            let item = edit_submenu
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
