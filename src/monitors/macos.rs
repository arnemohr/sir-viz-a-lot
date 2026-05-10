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

use core::ptr::NonNull;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CFRetained, CFString, CFUUID};
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

/// V31.2.3 — look up the cross-machine UUID for `display_id` via
/// `CGDisplayCreateUUIDFromDisplayID` (ColorSync / ApplicationServices).
///
/// `CGDisplayCreateUUIDFromDisplayID` follows the CoreFoundation "Create
/// rule": it returns a +1-retained `CFUUIDRef` that the caller owns.
/// We hand the raw pointer to `CFRetained::from_raw`, which takes ownership
/// and calls `CFRelease` on drop — no manual release needed.
///
/// The same `CFUUID::new_string` wrapper owns its `CFRetained<CFString>`,
/// so both objects are released when this function returns.
///
/// Returns `None` when:
/// - `display_id == 0` (`kCGNullDirectDisplay` — reserved; the function
///   checks this internally and returns null).
/// - The OS returns null for any other reason (disconnected display, race
///   with hot-plug).
///
/// This function is safe to call from any thread (no AppKit / main-thread
/// requirement, unlike NSScreen). The link target is ApplicationServices
/// rather than ColorSync directly: ColorSync only became a top-level
/// public framework in macOS 10.13 but has been a sub-framework of
/// ApplicationServices since 10.4. This mirrors winit-0.30's approach
/// (see winit `src/platform_impl/macos/ffi.rs`).
pub(super) fn uuid_for_display_id(display_id: u32) -> Option<String> {
    // `CGDisplayCreateUUIDFromDisplayID` lives in the ColorSync sub-framework
    // of ApplicationServices. Not exposed by `objc2-core-graphics 0.3`.
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C-unwind" {
        fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *mut CFUUID;
    }

    // SAFETY: `CGDisplayCreateUUIDFromDisplayID` is a well-documented C
    // function; we pass a plain `u32` and get back a nullable `CFUUIDRef`.
    // `CFRetained::from_raw` requires `NonNull` and takes Create-rule
    // ownership (calls `CFRelease` on drop).
    let raw: *mut CFUUID = unsafe { CGDisplayCreateUUIDFromDisplayID(display_id) };
    let uuid: CFRetained<CFUUID> = unsafe { CFRetained::from_raw(NonNull::new(raw)?) };

    // `CFUUID::new_string` wraps `CFUUIDCreateString` (Create rule).
    // The returned `CFRetained<CFString>` releases on drop.
    let cf_str: CFRetained<CFString> = CFUUID::new_string(None, Some(&uuid))?;
    let result = cf_str.to_string();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// V31.2.3 acceptance criterion (macOS): `uuid_for_display_id` must not
    /// panic for any `u32` input, including display_id 0 (historically
    /// `kCGNullDirectDisplay`) and `u32::MAX`. The result is `None` on a
    /// headless system or when the ID has no attached display, and `Some(_)`
    /// (a non-empty hyphenated UUID string) when a real display is connected.
    #[test]
    fn uuid_for_display_id_does_not_panic() {
        // Must never panic regardless of the returned value.
        let _ = uuid_for_display_id(0);
        let _ = uuid_for_display_id(u32::MAX);
    }

    /// V31.2.3 — when uuid_for_display_id returns Some, the string must be
    /// a non-empty, non-whitespace value (basic sanity; we don't validate the
    /// full hyphenated UUID format here since the exact layout is OS-chosen).
    #[test]
    fn uuid_for_display_id_some_is_non_empty() {
        // u32::MAX is never a real display ID; expect None from it.
        assert!(
            uuid_for_display_id(u32::MAX).is_none(),
            "u32::MAX display_id must yield None",
        );
        // For display_id 0: the result varies by machine (0 may map to the
        // primary display on some hardware, or return null on others). Just
        // verify it doesn't panic and, if Some, is non-empty.
        if let Some(uuid_str) = uuid_for_display_id(0) {
            assert!(
                !uuid_str.trim().is_empty(),
                "uuid_for_display_id(0) returned Some but the string is empty or whitespace",
            );
        }
    }
}
