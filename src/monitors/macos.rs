//! macOS-only NSScreen integration. Maps a `CGDirectDisplayID` (the
//! same value winit's `MonitorHandleExtMacOS::native_id()` returns) to
//! the human-readable display name reported by AppKit.
//!
//! 003-T2.7 — winit 0.30 returns `"Monitor #41052"`-style numeric
//! placeholders on macOS. AppKit's `NSScreen::localizedName()` returns
//! the actual product name (`"BenQ TH685"`, `"Built-in Display"`, …),
//! which the launcher's projector dropdown surfaces directly.
//!
//! Matching is done by `NSScreenNumber` rather than by frame geometry
//! because frame coordinates change with display arrangement and
//! resolution, while the `CGDirectDisplayID` is stable for the
//! lifetime of the display attachment.
//!
//! Pattern note: this module mirrors `src/show_day/sleep_assertion.rs`
//! in style — small, self-contained, no abstraction over an objc2
//! type that the rest of the codebase would have to learn. The
//! caller talks to a single free function.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSDictionary, NSNumber, NSString};

/// Look up the localized AppKit name for the screen whose
/// `CGDirectDisplayID` (a.k.a. `NSScreenNumber`) equals `target_id`.
///
/// Returns `None` when no attached screen matches, when the call
/// happens off the main thread (NSScreen requires a `MainThreadMarker`),
/// or when AppKit returns an empty string. The empty-string check is
/// belt-and-braces: external displays with their EDID stripped have
/// surfaced empty `localizedName` strings on early Apple Silicon
/// macOS releases; treating those as a miss lets the caller fall
/// through to the numeric `"Display N"` fallback.
pub fn localized_name_for_display_id(target_id: u32) -> Option<String> {
    // NSScreen is a `MainThreadOnly` AppKit class. The launcher and
    // monitor enumeration both run inside a winit `ApplicationHandler`
    // callback, which is on the main thread on macOS — so `new()`
    // succeeds in production. Defensively returning `None` keeps the
    // call site (a non-fatal name lookup) safe even if a future
    // refactor moves enumeration off-thread.
    let mtm = MainThreadMarker::new()?;
    let screens: Retained<objc2_foundation::NSArray<NSScreen>> = NSScreen::screens(mtm);
    for screen in screens.iter() {
        let id = screen_number(&screen)?;
        if id == target_id {
            let name_ns: Retained<NSString> = screen.localizedName();
            let name = name_ns.to_string();
            if name.is_empty() {
                return None;
            }
            return Some(name);
        }
    }
    None
}

/// Read the `NSScreenNumber` value from an `NSScreen`'s
/// `deviceDescription` dictionary. Apple's documentation guarantees
/// the value is an `NSNumber` boxing a `CGDirectDisplayID` (a `u32`).
///
/// Returns `None` if the dictionary is missing the key (very rare —
/// would indicate an AppKit bug or a synthetic test screen) or if the
/// value's class isn't `NSNumber` (shouldn't happen in practice).
fn screen_number(screen: &NSScreen) -> Option<u32> {
    let dict: Retained<NSDictionary<NSString, _>> = {
        // `deviceDescription` is typed as `NSDictionary<NSDeviceDescriptionKey, AnyObject>`
        // where `NSDeviceDescriptionKey == NSString`. The conversion is a
        // no-op cast under objc2's typed dictionary model.
        let raw = screen.deviceDescription();
        // SAFETY: `NSDeviceDescriptionKey` is a `pub type … = NSString;`
        // alias in objc2-app-kit. The `Retained<NSDictionary<…>>` wrapping
        // it is opaque to the type system but identical at the ObjC level.
        unsafe { Retained::cast_unchecked(raw) }
    };
    let key = NSString::from_str("NSScreenNumber");
    let value: Retained<objc2::runtime::AnyObject> = dict.objectForKey(&key)?;
    let number: &NSNumber = value.downcast_ref()?;
    Some(number.unsignedIntValue())
}
